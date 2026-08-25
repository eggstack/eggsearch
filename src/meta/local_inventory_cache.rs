use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Short freshness probe interval (checks status hash without full rebuild).
pub const FRESHNESS_PROBE_INTERVAL: Duration = Duration::from_secs(30);
/// Full inventory rebuild TTL.
pub const INVENTORY_REBUILD_TTL: Duration = Duration::from_secs(300);

use regex::Regex;
use xxhash_rust::xxh3::xxh3_64;

use crate::core::code_evidence::infer_source_role;
use crate::core::code_evidence::SourceRole;
use crate::core::code_evidence::SymbolKind;
use crate::core::local::{
    is_binary_extension, is_eligible_for_indexing, is_git_path_eligible, language_from_extension,
    should_skip_component, LocalConfig,
};
use crate::meta::local_ignore::IgnoreStack;
use crate::meta::local_inventory::{read_head_commit, resolve_git_dir};

const BUILD_TIMEOUT_SECS: u64 = 5;
const INVENTORY_BUILD_TIMEOUT: Duration = Duration::from_secs(BUILD_TIMEOUT_SECS);

const GIT_STDOUT_CAP: usize = 16 * 1024 * 1024;
const GIT_STDERR_CAP: usize = 64 * 1024;

const TRIGGER_TIMEOUT: u8 = 0;
const TRIGGER_STDOUT_LIMIT: u8 = 1;
const TRIGGER_STDERR_LIMIT: u8 = 2;

struct ProcessTerminationController {
    child_pgid: i32,
    trigger: AtomicU8,
    kill_sent: AtomicBool,
}

impl ProcessTerminationController {
    fn new(child_pgid: i32) -> Self {
        Self {
            child_pgid,
            trigger: AtomicU8::new(u8::MAX),
            kill_sent: AtomicBool::new(false),
        }
    }

    fn try_terminate(&self, trigger: u8) -> bool {
        if self
            .kill_sent
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.trigger.store(trigger, Ordering::Relaxed);
            unsafe {
                libc::kill(-self.child_pgid, libc::SIGKILL);
            }
            true
        } else {
            false
        }
    }

    fn termination_reason(&self) -> CommandTermination {
        match self.trigger.load(Ordering::Relaxed) {
            TRIGGER_TIMEOUT => CommandTermination::TimedOut,
            TRIGGER_STDOUT_LIMIT => CommandTermination::StdoutLimitExceeded,
            TRIGGER_STDERR_LIMIT => CommandTermination::StderrLimitExceeded,
            _ if self.kill_sent.load(Ordering::Relaxed) => CommandTermination::Signaled,
            _ => CommandTermination::Exited,
        }
    }
}

/// A single file entry in the workspace inventory.
#[derive(Clone, Debug)]
pub struct InventoryEntry {
    /// Root index this entry belongs to.
    pub root_index: usize,
    /// Root-relative path.
    pub relative_path: String,
    /// Absolute path to the file.
    pub absolute_path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Detected language from file extension.
    pub language: Option<String>,
    /// Inferred source role (test, implementation, etc.).
    pub role: SourceRole,
    /// Whether the file is a known binary extension.
    pub is_binary: bool,
    /// Last modified time as seconds since Unix epoch.
    pub mtime_secs: u64,
    /// Fingerprint combining path, size, and mtime for change detection.
    pub fingerprint: u64,
}

/// Inventory for a single root directory.
#[derive(Clone, Debug)]
pub struct RootInventory {
    /// Root index.
    pub root_index: usize,
    /// Canonical root path.
    pub root_path: PathBuf,
    /// File entries discovered under this root.
    pub entries: Vec<InventoryEntry>,
    /// When this inventory was built.
    pub built_at: Instant,
    /// HEAD commit SHA at build time, if a git repo.
    pub head_commit: Option<String>,
    /// Total number of entries.
    pub entry_count: usize,
    /// Whether the inventory was truncated by limits.
    pub truncated: bool,
    /// Reason for truncation, if any.
    pub truncation_reason: Option<String>,
    /// Whether the git backend was used for this root.
    pub uses_git_backend: bool,
    /// Number of untracked files included via --others flag.
    pub untracked_count: usize,
    /// Git index file mtime at build time (seconds since epoch), if applicable.
    pub index_mtime_secs: Option<u64>,
    /// Hash of `git status --porcelain` output for change detection.
    pub status_hash: Option<u64>,
}

/// Workspace-level inventory aggregating all roots.
#[derive(Clone, Debug)]
pub struct WorkspaceInventory {
    /// Per-root inventories.
    pub roots: Vec<RootInventory>,
    /// Total entries across all roots.
    pub total_entries: usize,
    /// When this inventory was built.
    pub built_at: Instant,
    /// Fingerprint of the config used to build this inventory.
    pub config_fingerprint: u64,
}

static SYMBOL_PATTERNS: LazyLock<Vec<(Regex, SymbolKind)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)").unwrap(),
            SymbolKind::Function,
        ),
        (
            Regex::new(r"(?:pub\s+)?struct\s+(\w+)").unwrap(),
            SymbolKind::Struct,
        ),
        (
            Regex::new(r"(?:pub\s+)?enum\s+(\w+)").unwrap(),
            SymbolKind::Enum,
        ),
        (
            Regex::new(r"(?:pub\s+)?trait\s+(\w+)").unwrap(),
            SymbolKind::Trait,
        ),
        (
            Regex::new(r"impl(?:<[^>]*>)?\s+(?:dyn\s+)?(\w+)").unwrap(),
            SymbolKind::Struct,
        ),
        (
            Regex::new(r"(?:pub\s+)?type\s+(\w+)").unwrap(),
            SymbolKind::TypeAlias,
        ),
        (
            Regex::new(r"macro_rules!\s+(\w+)").unwrap(),
            SymbolKind::Macro,
        ),
        (
            Regex::new(r"(?:pub\s+)?(?:const|static)\s+(?:\w+\s*:)?\s*(\w+)").unwrap(),
            SymbolKind::Constant,
        ),
        (
            Regex::new(r"(?:async\s+)?def\s+(\w+)").unwrap(),
            SymbolKind::Function,
        ),
        (
            Regex::new(r"class\s+(\w+)").unwrap(),
            SymbolKind::Class,
        ),
        (
            Regex::new(r"(?:export\s+)?(?:async\s+)?function\s+(\w+)").unwrap(),
            SymbolKind::Function,
        ),
        (
            Regex::new(r"(?:export\s+)?class\s+(\w+)").unwrap(),
            SymbolKind::Class,
        ),
        (
            Regex::new(r"(?:export\s+)?interface\s+(\w+)").unwrap(),
            SymbolKind::Interface,
        ),
        (
            Regex::new(r"(?:export\s+)?type\s+(\w+)").unwrap(),
            SymbolKind::TypeAlias,
        ),
        (
            Regex::new(r"func\s+(?:\([^)]*\)\s+)?(\w+)").unwrap(),
            SymbolKind::Function,
        ),
        (
            Regex::new(r"type\s+(\w+)\s+(?:struct|interface)").unwrap(),
            SymbolKind::Struct,
        ),
        (
            Regex::new(r"(?:public|private|protected|internal)\s+(?:static\s+)?(?:class|interface|enum)\s+(\w+)").unwrap(),
            SymbolKind::Class,
        ),
        (
            Regex::new(r"(?:typedef\s+)?(?:struct|class)\s+(\w+)").unwrap(),
            SymbolKind::Struct,
        ),
        (
            Regex::new(r"^(?:static|inline|extern|const)?\s*\w[\w\s\*]*\b(\w+)\s*\(").unwrap(),
            SymbolKind::Function,
        ),
    ]
});

/// Scan text for symbol definitions matching the hint.
pub fn find_symbols_in_text(text: &str, symbol_hint: &str) -> Option<(String, SymbolKind, u32)> {
    let hint_lower = symbol_hint.to_lowercase();

    for (line_idx, line) in text.lines().enumerate() {
        let line_lower = line.to_lowercase();
        if !line_lower.contains(&hint_lower) {
            continue;
        }
        for (re, kind) in SYMBOL_PATTERNS.iter() {
            if let Some(caps) = re.captures(line) {
                if let Some(name_match) = caps.get(1) {
                    let name = name_match.as_str();
                    if name.to_lowercase() == hint_lower {
                        let line_num = u32::try_from(line_idx + 1).unwrap_or(u32::MAX);
                        return Some((name.to_string(), *kind, line_num));
                    }
                }
            }
        }
    }
    None
}

/// Score an inventory entry against a query without reading file content.
pub fn score_inventory_entry(
    entry: &InventoryEntry,
    query_lower: &str,
    query_tokens: &[&str],
) -> f64 {
    let mut score: f64 = 0.0;
    let path_lower = entry.relative_path.to_lowercase();
    let file_name = Path::new(&entry.relative_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if file_name == query_lower {
        score += 100.0;
    }

    for token in query_tokens {
        if path_lower.contains(token) {
            score += 20.0;
        }
    }

    if score > 0.0 && entry.language.is_some() {
        score += 5.0;
    }

    let penalty_extensions = [".lock", "min.js", "min.css", ".map"];
    for ext in &penalty_extensions {
        if file_name.ends_with(ext) {
            score -= 150.0;
        }
    }

    match entry.role {
        SourceRole::Implementation => score += 10.0,
        SourceRole::Test => score += 5.0,
        SourceRole::Example => score += 5.0,
        SourceRole::Documentation => score += 8.0,
        SourceRole::Readme => score += 12.0,
        _ => {}
    }

    score
}

fn compute_fingerprint(path: &str, size: u64, mtime_secs: u64) -> u64 {
    let mut data = Vec::with_capacity(path.len() + 16);
    data.extend_from_slice(path.as_bytes());
    data.extend_from_slice(&size.to_le_bytes());
    data.extend_from_slice(&mtime_secs.to_le_bytes());
    xxh3_64(&data)
}

fn compute_config_fingerprint(config: &LocalConfig) -> u64 {
    let mut data = Vec::new();
    data.extend_from_slice(&config.max_file_bytes.to_le_bytes());
    data.extend_from_slice(&config.max_indexed_files.to_le_bytes());
    data.push(config.include_hidden as u8);
    data.push(config.respect_gitignore as u8);
    data.push(config.follow_symlinks as u8);
    for root in &config.roots {
        data.extend_from_slice(root.to_string_lossy().as_bytes());
        data.push(0);
    }
    xxh3_64(&data)
}

fn mtime_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a workspace inventory from configured roots and local config.
pub fn build_inventory(config: &LocalConfig, roots: &[(usize, PathBuf)]) -> WorkspaceInventory {
    let config_fingerprint = compute_config_fingerprint(config);
    let mut root_inventories = Vec::with_capacity(roots.len());
    let mut total_entries = 0usize;

    for &(root_index, ref root_path) in roots {
        let ri = build_inventory_for_root(root_index, root_path, config);
        total_entries += ri.entry_count;
        root_inventories.push(ri);
    }

    WorkspaceInventory {
        roots: root_inventories,
        total_entries,
        built_at: Instant::now(),
        config_fingerprint,
    }
}

fn build_inventory_for_root(
    root_index: usize,
    root_path: &Path,
    config: &LocalConfig,
) -> RootInventory {
    if let Some(ri) = build_inventory_git(root_index, root_path, config) {
        return ri;
    }

    build_inventory_native(root_index, root_path, config)
}

/// Build inventory for a single root using native filesystem walking.
pub fn build_inventory_native(
    root_index: usize,
    root_path: &Path,
    config: &LocalConfig,
) -> RootInventory {
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut truncation_reason = None;
    let start = Instant::now();
    let ignore_stack = if config.respect_gitignore {
        IgnoreStack::build(root_path, root_path)
    } else {
        IgnoreStack::new()
    };

    walk_native(
        root_path,
        root_path,
        root_index,
        config,
        &ignore_stack,
        &mut entries,
        &start,
        &mut truncated,
        &mut truncation_reason,
    );

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let entry_count = entries.len();
    let head_commit = read_head_commit(root_path);

    RootInventory {
        root_index,
        root_path: root_path.to_path_buf(),
        entries,
        built_at: Instant::now(),
        head_commit,
        entry_count,
        truncated,
        truncation_reason,
        uses_git_backend: false,
        untracked_count: 0,
        index_mtime_secs: None,
        status_hash: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_native(
    dir: &Path,
    root_path: &Path,
    root_index: usize,
    config: &LocalConfig,
    ignore_stack: &IgnoreStack,
    entries: &mut Vec<InventoryEntry>,
    start: &Instant,
    truncated: &mut bool,
    truncation_reason: &mut Option<String>,
) {
    if start.elapsed() > INVENTORY_BUILD_TIMEOUT {
        *truncated = true;
        *truncation_reason = Some("build timeout exceeded".to_string());
        return;
    }

    if entries.len() >= config.max_indexed_files {
        *truncated = true;
        *truncation_reason = Some(format!(
            "max_indexed_files limit ({}) reached",
            config.max_indexed_files
        ));
        return;
    }

    let read_dir = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "failed to read directory");
            return;
        }
    };

    let mut dir_entries: Vec<_> = read_dir.flatten().collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        if start.elapsed() > INVENTORY_BUILD_TIMEOUT {
            *truncated = true;
            *truncation_reason = Some("build timeout exceeded".to_string());
            return;
        }
        if *truncated {
            return;
        }
        if entries.len() >= config.max_indexed_files {
            *truncated = true;
            *truncation_reason = Some(format!(
                "max_indexed_files limit ({}) reached",
                config.max_indexed_files
            ));
            return;
        }

        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if should_skip_component(&file_name_str, config.include_hidden) {
            continue;
        }

        let path = entry.path();

        if !config.follow_symlinks {
            if let Ok(meta) = std::fs::symlink_metadata(&path) {
                if meta.file_type().is_symlink() {
                    continue;
                }
            }
        }

        if config.respect_gitignore {
            let is_dir = path.is_dir();
            if ignore_stack.is_ignored(root_path, &path, is_dir) {
                continue;
            }
        }

        if path.is_dir() {
            walk_native(
                &path,
                root_path,
                root_index,
                config,
                ignore_stack,
                entries,
                start,
                truncated,
                truncation_reason,
            );
        } else if path.is_file() {
            if let Some(entry) = build_entry_for_file(&path, root_path, root_index, config) {
                entries.push(entry);
            }
        }
    }
}

fn build_entry_for_file(
    path: &Path,
    root_path: &Path,
    root_index: usize,
    config: &LocalConfig,
) -> Option<InventoryEntry> {
    if !is_eligible_for_indexing(path, config) {
        return None;
    }

    let metadata = std::fs::metadata(path).ok()?;
    let relative_path = path
        .strip_prefix(root_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let language = language_from_extension(&relative_path).map(|s| s.to_string());
    let role = infer_source_role(&relative_path);
    let is_binary = is_binary_extension(&relative_path);
    let size = metadata.len();
    let mtime = mtime_secs(path);
    let fingerprint = compute_fingerprint(&relative_path, size, mtime);

    Some(InventoryEntry {
        root_index,
        relative_path,
        absolute_path: path.to_path_buf(),
        size,
        language,
        role,
        is_binary,
        mtime_secs: mtime,
        fingerprint,
    })
}

/// How the bounded command was terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTermination {
    /// Process exited normally.
    Exited,
    /// Process was killed due to timeout.
    TimedOut,
    /// Stdout cap breach triggered termination.
    StdoutLimitExceeded,
    /// Stderr cap breach triggered termination.
    StderrLimitExceeded,
    /// Process could not be spawned.
    SpawnFailed,
    /// Process was killed by a signal.
    Signaled,
}

/// Build inventory for a single root using `git ls-files` (returns `None` for non-git dirs).
#[allow(dead_code)]
#[cfg(not(feature = "mock"))]
struct BoundedCommandResult {
    status: Option<std::process::ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    termination: CommandTermination,
}

#[allow(dead_code)]
#[cfg(feature = "mock")]
/// Result of a bounded command execution.
pub struct BoundedCommandResult {
    /// The exit status of the command, if it was spawned.
    pub status: Option<std::process::ExitStatus>,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
    /// Whether the command was killed due to timeout.
    pub timed_out: bool,
    /// Whether stdout was truncated at the cap.
    pub stdout_truncated: bool,
    /// Whether stderr was truncated at the cap.
    pub stderr_truncated: bool,
    /// How the command was terminated.
    pub termination: CommandTermination,
}

#[cfg(feature = "mock")]
pub mod test_harness {
    //! Test harness for the bounded command runner infrastructure.
    //! Gated behind the `mock` feature so downstream binaries don't link test code.

    use super::*;

    /// Run a command with timeout, stdout/stderr caps, and process group management.
    pub fn run(cmd: &mut Command, timeout: Duration) -> BoundedCommandResult {
        run_bounded_command_impl(cmd, timeout, GIT_STDOUT_CAP, GIT_STDERR_CAP)
    }

    /// Run a command with inventory-specific caps.
    pub fn run_for_inventory(
        cmd: &mut Command,
        timeout: Duration,
        cap: usize,
    ) -> BoundedCommandResult {
        run_bounded_command_impl(cmd, timeout, cap, GIT_STDERR_CAP)
    }

    /// The default inventory build timeout.
    pub const INVENTORY_TIMEOUT: Duration = INVENTORY_BUILD_TIMEOUT;
    /// The default stdout cap (16 MB).
    pub const STDOUT_CAP: usize = GIT_STDOUT_CAP;
    /// The default stderr cap (64 KB).
    pub const STDERR_CAP: usize = GIT_STDERR_CAP;
}

fn run_bounded_command(cmd: &mut Command, timeout: Duration) -> BoundedCommandResult {
    run_bounded_command_impl(cmd, timeout, GIT_STDOUT_CAP, GIT_STDERR_CAP)
}

fn run_bounded_command_for_inventory(
    cmd: &mut Command,
    timeout: Duration,
    cap: usize,
) -> BoundedCommandResult {
    run_bounded_command_impl(cmd, timeout, cap, GIT_STDERR_CAP)
}

pub(crate) fn run_bounded_for_discovery(
    cmd: &mut Command,
    timeout: Duration,
    stdout_cap: usize,
) -> Option<(bool, Vec<u8>)> {
    let result = run_bounded_command_impl(cmd, timeout, stdout_cap, GIT_STDERR_CAP);
    let status = result.status?;
    Some((status.success(), result.stdout))
}

fn run_bounded_command_impl(
    cmd: &mut Command,
    timeout: Duration,
    stdout_cap: usize,
    stderr_cap: usize,
) -> BoundedCommandResult {
    use std::io::Read;

    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", "*");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }

    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            return BoundedCommandResult {
                status: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                stdout_truncated: false,
                stderr_truncated: false,
                termination: CommandTermination::SpawnFailed,
            };
        }
    };

    let child_id = child.id() as i32;
    let controller = Arc::new(ProcessTerminationController::new(child_id));
    let exited = Arc::new(AtomicBool::new(false));

    let controller_timeout = controller.clone();
    let exited_timeout = exited.clone();
    let kill_handle = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if exited_timeout.load(Ordering::Relaxed) {
                return;
            }
            if std::time::Instant::now() >= deadline {
                controller_timeout.try_terminate(TRIGGER_TIMEOUT);
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let controller_stdout = controller.clone();
    let stdout_thread = std::thread::spawn(move || {
        let mut local_stdout = Vec::new();
        let mut local_truncated = false;
        if let Some(mut out) = stdout_handle {
            let mut buf = [0u8; 8192];
            loop {
                match out.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let remaining = stdout_cap.saturating_sub(local_stdout.len());
                        if n <= remaining {
                            local_stdout.extend_from_slice(&buf[..n]);
                        } else {
                            local_stdout.extend_from_slice(&buf[..remaining]);
                            local_truncated = true;
                            controller_stdout.try_terminate(TRIGGER_STDOUT_LIMIT);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (local_stdout, local_truncated)
    });

    let controller_stderr = controller.clone();
    let stderr_thread = std::thread::spawn(move || {
        let mut local_stderr = Vec::new();
        let mut local_truncated = false;
        if let Some(mut err) = stderr_handle {
            let mut buf = [0u8; 8192];
            loop {
                match err.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let remaining = stderr_cap.saturating_sub(local_stderr.len());
                        if n <= remaining {
                            local_stderr.extend_from_slice(&buf[..n]);
                        } else {
                            local_stderr.extend_from_slice(&buf[..remaining]);
                            local_truncated = true;
                            controller_stderr.try_terminate(TRIGGER_STDERR_LIMIT);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (local_stderr, local_truncated)
    });

    let (stdout, stdout_truncated) = stdout_thread.join().unwrap_or((Vec::new(), false));
    let (stderr, stderr_truncated) = stderr_thread.join().unwrap_or((Vec::new(), false));

    let status = child.wait().ok();
    exited.store(true, Ordering::Relaxed);
    let _ = kill_handle.join();

    let timed_out = controller.trigger.load(Ordering::Relaxed) == TRIGGER_TIMEOUT;
    let termination = controller.termination_reason();

    BoundedCommandResult {
        status,
        stdout,
        stderr,
        timed_out,
        stdout_truncated,
        stderr_truncated,
        termination,
    }
}

/// Build inventory for a single root using `git ls-files` with bounded command execution.
/// Returns `None` for non-git directories, timeouts, or output exceeding limits.
pub fn build_inventory_git(
    root_index: usize,
    root_path: &Path,
    config: &LocalConfig,
) -> Option<RootInventory> {
    let mut cmd = Command::new("git");
    cmd.arg("ls-files")
        .arg("-z")
        .arg("--cached")
        .arg("--others")
        .arg("--exclude-standard")
        .current_dir(root_path);

    let result = run_bounded_command(&mut cmd, INVENTORY_BUILD_TIMEOUT);

    if result.timed_out {
        tracing::warn!(
            root = %root_path.display(),
            "git ls-files timed out, falling back to native walker"
        );
        return None;
    }

    let status = result.status?;
    if !status.success() {
        return None;
    }

    if result.stdout.len() >= GIT_STDOUT_CAP {
        tracing::warn!(
            root = %root_path.display(),
            stdout_len = result.stdout.len(),
            "git ls-files output exceeded cap, falling back to native walker"
        );
        return None;
    }

    let head_commit = read_head_commit(root_path);
    let start = Instant::now();
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut truncation_reason = None;
    let mut untracked_count = 0usize;

    let mut untracked_cmd = Command::new("git");
    untracked_cmd
        .arg("ls-files")
        .arg("-z")
        .arg("--others")
        .arg("--exclude-standard")
        .current_dir(root_path);
    let untracked_result = run_bounded_command_for_inventory(
        &mut untracked_cmd,
        INVENTORY_BUILD_TIMEOUT,
        GIT_STDOUT_CAP,
    );
    if untracked_result
        .status
        .as_ref()
        .is_some_and(|s| s.success())
    {
        untracked_count = untracked_result
            .stdout
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .count();
    }

    for path_bytes in result.stdout.split(|&b| b == 0) {
        if path_bytes.is_empty() {
            continue;
        }
        if start.elapsed() > INVENTORY_BUILD_TIMEOUT {
            truncated = true;
            truncation_reason = Some("build timeout exceeded".to_string());
            break;
        }
        if entries.len() >= config.max_indexed_files {
            truncated = true;
            truncation_reason = Some(format!(
                "max_indexed_files limit ({}) reached",
                config.max_indexed_files
            ));
            break;
        }

        let line = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if !is_git_path_eligible(line, root_path, config) {
            continue;
        }

        let absolute_path = root_path.join(line);
        let metadata = match std::fs::metadata(&absolute_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();
        let mtime = mtime_secs(&absolute_path);
        let language = language_from_extension(line).map(|s| s.to_string());
        let role = infer_source_role(line);
        let is_binary = is_binary_extension(line);
        let fingerprint = compute_fingerprint(line, size, mtime);

        entries.push(InventoryEntry {
            root_index,
            relative_path: line.to_string(),
            absolute_path,
            size,
            language,
            role,
            is_binary,
            mtime_secs: mtime,
            fingerprint,
        });
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let entry_count = entries.len();
    let index_mtime = resolve_git_dir(root_path).and_then(|git_dir| {
        let index_path = git_dir.join("index");
        index_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
    });

    let mut status_hash = None;
    let mut status_cmd = Command::new("git");
    status_cmd
        .arg("status")
        .arg("--porcelain=v2")
        .arg("-z")
        .arg("--untracked-files=normal")
        .current_dir(root_path);
    let status_result =
        run_bounded_command_for_inventory(&mut status_cmd, INVENTORY_BUILD_TIMEOUT, GIT_STDOUT_CAP);
    if status_result.status.as_ref().is_some_and(|s| s.success()) && !status_result.stdout_truncated
    {
        use xxhash_rust::xxh3::xxh3_64;
        status_hash = Some(xxh3_64(&status_result.stdout));
    }

    Some(RootInventory {
        root_index,
        root_path: root_path.to_path_buf(),
        entries,
        built_at: Instant::now(),
        head_commit,
        entry_count,
        truncated,
        truncation_reason,
        uses_git_backend: true,
        untracked_count,
        index_mtime_secs: index_mtime,
        status_hash,
    })
}

/// Check whether the inventory needs rebuilding based on age, config changes, or git HEAD.
pub fn needs_rebuild(
    inventory: &WorkspaceInventory,
    config: &LocalConfig,
    stale_threshold: Duration,
) -> bool {
    let age = inventory.built_at.elapsed();
    if age > stale_threshold {
        return true;
    }

    let current_fingerprint = compute_config_fingerprint(config);
    if inventory.config_fingerprint != current_fingerprint {
        return true;
    }

    for ri in &inventory.roots {
        if ri.uses_git_backend {
            if let Some(current_head) = read_head_commit(&ri.root_path) {
                if ri.head_commit.as_ref() != Some(&current_head) {
                    return true;
                }
            }
            if ri.status_hash.is_some() {
                let mut status_cmd = Command::new("git");
                status_cmd
                    .arg("status")
                    .arg("--porcelain=v2")
                    .arg("-z")
                    .arg("--untracked-files=normal")
                    .current_dir(&ri.root_path);
                let status_result = run_bounded_command_for_inventory(
                    &mut status_cmd,
                    INVENTORY_BUILD_TIMEOUT,
                    GIT_STDOUT_CAP,
                );
                if status_result.status.as_ref().is_some_and(|s| s.success())
                    && !status_result.stdout_truncated
                {
                    use xxhash_rust::xxh3::xxh3_64;
                    let current_hash = xxh3_64(&status_result.stdout);
                    if Some(current_hash) != ri.status_hash {
                        return true;
                    }
                    // Status hash matches — working tree state is identical;
                    // skip the index_mtime check (status_hash is authoritative).
                    continue;
                }
            }
            // Fallback: check index_mtime when status_hash is unavailable.
            if let Some(stored_mtime) = ri.index_mtime_secs {
                if let Some(git_dir) = resolve_git_dir(&ri.root_path) {
                    let index_path = git_dir.join("index");
                    let current_mtime = std::fs::metadata(&index_path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());
                    if let Some(current_mtime) = current_mtime {
                        if current_mtime != stored_mtime {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Lightweight freshness probe: checks status hash only (no age/config/HEAD checks).
/// Returns `true` if the status hash has changed, meaning the inventory is stale.
/// Returns `false` if the status hash is unchanged (inventory is still fresh).
/// Returns `true` if status hash is unavailable (conservative: assume stale).
pub fn probe_needs_rebuild(inventory: &WorkspaceInventory) -> bool {
    for ri in &inventory.roots {
        if ri.uses_git_backend {
            if let Some(stored_hash) = ri.status_hash {
                let mut status_cmd = Command::new("git");
                status_cmd
                    .arg("status")
                    .arg("--porcelain=v2")
                    .arg("-z")
                    .arg("--untracked-files=normal")
                    .current_dir(&ri.root_path);
                let status_result = run_bounded_command_for_inventory(
                    &mut status_cmd,
                    INVENTORY_BUILD_TIMEOUT,
                    GIT_STDOUT_CAP,
                );
                if status_result.status.as_ref().is_some_and(|s| s.success())
                    && !status_result.stdout_truncated
                {
                    let current_hash = xxh3_64(&status_result.stdout);
                    if Some(current_hash) != Some(stored_hash) {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }
    }
    false
}

/// Validate that inventory entries still match filesystem state.
pub fn validate_entries(inventory: &RootInventory, config: &LocalConfig) -> Vec<bool> {
    inventory
        .entries
        .iter()
        .map(|entry| validate_entry(entry, config))
        .collect()
}

pub(crate) fn validate_entry(entry: &InventoryEntry, config: &LocalConfig) -> bool {
    let metadata = match std::fs::metadata(&entry.absolute_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    if !metadata.is_file() {
        return false;
    }

    if metadata.len() > config.max_file_bytes as u64 {
        return false;
    }

    let current_mtime = entry
        .absolute_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let current_fingerprint =
        compute_fingerprint(&entry.relative_path, metadata.len(), current_mtime);

    entry.fingerprint == current_fingerprint
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}",
        )
        .unwrap();
        fs::write(
            root.join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .unwrap();
        fs::write(root.join("README.md"), "# My Project\n\nA test project.").unwrap();
        fs::write(root.join("config.toml"), "[server]\nport = 8080").unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/engine.rs"),
            "pub struct Engine {\n    name: String,\n}",
        )
        .unwrap();
        fs::write(root.join("src/utils.rs"), "pub fn helper() -> i32 { 42 }").unwrap();

        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/integration.rs"),
            "#[test]\nfn test_add() { assert_eq!(1 + 1, 2); }",
        )
        .unwrap();

        fs::write(root.join(".hidden"), "secret").unwrap();
        fs::write(root.join("data.bin"), vec![0u8; 100]).unwrap();

        dir
    }

    #[test]
    fn score_inventory_entry_does_not_penalize_non_lock_filenames() {
        let entry = InventoryEntry {
            root_index: 0,
            relative_path: "scripts/deadlock".to_string(),
            absolute_path: PathBuf::from("/ws/scripts/deadlock"),
            size: 10,
            language: Some("python".to_string()),
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 0,
            fingerprint: 0,
        };
        let score = score_inventory_entry(&entry, "deadlock", &["deadlock"]);
        assert!(
            score > 0.0,
            "filename ending in 'lock' without .lock extension must not be penalized: {score}"
        );
    }

    #[test]
    fn score_inventory_entry_penalizes_lock_files() {
        let entry = InventoryEntry {
            root_index: 0,
            relative_path: "Cargo.lock".to_string(),
            absolute_path: PathBuf::from("/ws/Cargo.lock"),
            size: 10,
            language: None,
            role: SourceRole::Implementation,
            is_binary: false,
            mtime_secs: 0,
            fingerprint: 0,
        };
        let score = score_inventory_entry(&entry, "cargo.lock", &["cargo.lock"]);
        assert!(score < 0.0, "expected lock-file penalty: {score}");
    }

    fn default_config() -> LocalConfig {
        LocalConfig {
            enabled: true,
            roots: Vec::new(),
            max_file_bytes: 1_048_576,
            max_indexed_files: 50_000,
            include_hidden: false,
            respect_gitignore: true,
            follow_symlinks: false,
        }
    }

    #[test]
    fn test_build_inventory_native_basic() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root.clone())];

        let inventory = build_inventory(&config, &roots);
        assert_eq!(inventory.roots.len(), 1);
        let ri = &inventory.roots[0];
        assert!(ri.entry_count > 0);
        assert!(!ri.uses_git_backend);
        assert!(ri.entries.iter().all(|e| e.root_index == 0));

        let rel_paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(rel_paths.contains(&"main.rs"));
        assert!(rel_paths.contains(&"lib.rs"));
        assert!(rel_paths.contains(&"README.md"));
        assert!(rel_paths.contains(&"config.toml"));
        assert!(rel_paths.contains(&"src/engine.rs"));
        assert!(rel_paths.contains(&"src/utils.rs"));
        assert!(rel_paths.contains(&"tests/integration.rs"));

        assert!(!rel_paths.contains(&".hidden"));
        assert!(!rel_paths.contains(&"data.bin"));
    }

    #[test]
    fn test_build_inventory_respects_max_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..20 {
            fs::write(
                root.join(format!("file_{i:03}.rs")),
                format!("fn f_{i}() {{}}"),
            )
            .unwrap();
        }
        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            max_file_bytes: 1_048_576,
            max_indexed_files: 10,
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
        };
        let roots = vec![(0, root.to_path_buf())];
        let inventory = build_inventory(&config, &roots);
        let ri = &inventory.roots[0];
        assert!(
            ri.entry_count <= 10,
            "entry_count {} should be <= 10",
            ri.entry_count
        );
        assert!(ri.truncated);
        assert!(ri.truncation_reason.is_some());
    }

    #[test]
    fn test_inventory_deterministic_ordering() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root.clone())];

        let inv1 = build_inventory(&config, &roots);
        let inv2 = build_inventory(&config, &roots);

        let paths1: Vec<&str> = inv1.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        let paths2: Vec<&str> = inv2.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert_eq!(paths1, paths2);
    }

    #[test]
    fn test_needs_rebuild_fresh() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root)];

        let inventory = build_inventory(&config, &roots);
        let threshold = Duration::from_secs(30);
        assert!(!needs_rebuild(&inventory, &config, threshold));
    }

    #[test]
    fn test_needs_rebuild_stale() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root)];

        let mut inventory = build_inventory(&config, &roots);
        inventory.built_at = Instant::now() - Duration::from_secs(31);
        let threshold = Duration::from_secs(30);
        assert!(needs_rebuild(&inventory, &config, threshold));
    }

    #[test]
    fn test_validate_entries_existing_file() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root)];

        let inventory = build_inventory(&config, &roots);
        let ri = &inventory.roots[0];
        let results = validate_entries(ri, &config);
        assert!(results.iter().all(|&v| v));
    }

    #[test]
    fn test_validate_entries_missing_file() {
        let dir = make_temp_workspace();
        let root = dir.path().canonicalize().unwrap();
        let config = default_config();
        let roots = vec![(0, root)];

        let mut inventory = build_inventory(&config, &roots);
        let ri = &mut inventory.roots[0];

        if let Some(entry) = ri.entries.first() {
            let missing_path = entry.absolute_path.clone();
            let mut modified_entry = entry.clone();
            modified_entry.absolute_path = missing_path;
            modified_entry.relative_path = "nonexistent_file_xyz.rs".to_string();
            ri.entries.push(modified_entry);
        }

        let results = validate_entries(ri, &config);
        let has_false = results.iter().any(|&v| !v);
        assert!(has_false);
    }

    #[test]
    fn test_git_inventory_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("hello.rs"), "fn main() {}").unwrap();

        let config = default_config();
        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_none(), "non-git dir should return None");

        let ri_native = build_inventory_native(0, root, &config);
        assert!(!ri_native.uses_git_backend);
        assert_eq!(ri_native.entry_count, 1);
    }

    #[test]
    fn test_inventory_respects_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("visible.rs"), "fn main() {}").unwrap();
        fs::write(root.join(".hidden_file"), "secret").unwrap();
        fs::create_dir_all(root.join(".hidden_dir")).unwrap();
        fs::write(root.join(".hidden_dir/nested.rs"), "fn nested() {}").unwrap();

        let config_no_hidden = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            max_file_bytes: 1_048_576,
            max_indexed_files: 50_000,
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv = build_inventory(&config_no_hidden, &roots);
        let paths: Vec<&str> = inv.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"visible.rs"));
        assert!(!paths.contains(&".hidden_file"));
        assert!(!paths.iter().any(|p| p.starts_with(".hidden_dir/")));
    }

    #[test]
    fn test_inventory_respects_skip_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target/debug.rs"), "fn debug() {}").unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/pkg.js"), "// pkg").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            max_file_bytes: 1_048_576,
            max_indexed_files: 50_000,
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv = build_inventory(&config, &roots);
        let paths: Vec<&str> = inv.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"main.rs"));
        assert!(!paths.iter().any(|p| p.starts_with("target/")));
        assert!(!paths.iter().any(|p| p.starts_with("node_modules/")));
    }

    #[test]
    fn test_inventory_binary_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("source.rs"), "fn main() {}").unwrap();
        fs::write(root.join("image.png"), vec![0u8; 50]).unwrap();
        fs::write(root.join("archive.zip"), vec![0u8; 50]).unwrap();
        fs::write(root.join("doc.pdf"), vec![0u8; 50]).unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            max_file_bytes: 1_048_576,
            max_indexed_files: 50_000,
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv = build_inventory(&config, &roots);
        let paths: Vec<&str> = inv.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"source.rs"));
        assert!(!paths.contains(&"image.png"));
        assert!(!paths.contains(&"archive.zip"));
        assert!(!paths.contains(&"doc.pdf"));
    }

    #[test]
    fn test_first_search_builds_inventory() {
        use crate::meta::local_backend::{LocalWorkspaceBackend, RegexSymbolBackend};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("lib.rs"), "pub fn add() -> i32 { 1 }").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let roots = vec![(0, root.to_path_buf())];
        let cache = std::sync::RwLock::new(None);

        let result = LocalWorkspaceBackend::search_sync(
            &config,
            &roots,
            "main",
            None,
            None,
            None,
            None,
            10,
            Duration::from_secs(10),
            Instant::now(),
            &cache,
            &RegexSymbolBackend,
        );

        assert!(result.files_scanned > 0);
        let telemetry = result.telemetry.as_ref().unwrap();
        assert!(telemetry.used_inventory);
        assert!(telemetry.cold_build);
        assert!(telemetry.inventory_build_time_ms > 0);

        let cached = cache.read().unwrap();
        assert!(cached.is_some());
        let inv = cached.as_ref().unwrap();
        assert!(inv.roots[0].entry_count >= 2);
    }

    #[test]
    fn test_second_search_reuses_inventory() {
        use crate::meta::local_backend::{LocalWorkspaceBackend, RegexSymbolBackend};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("lib.rs"), "pub fn add() -> i32 { 1 }").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let roots = vec![(0, root.to_path_buf())];
        let cache = std::sync::RwLock::new(None);

        let search_config = config.clone();
        let search_roots = roots.clone();
        let cache_ref = &cache;

        let _r1 = LocalWorkspaceBackend::search_sync(
            &search_config,
            &search_roots,
            "main",
            None,
            None,
            None,
            None,
            10,
            Duration::from_secs(10),
            Instant::now(),
            cache_ref,
            &RegexSymbolBackend,
        );

        let start2 = Instant::now();
        let _r2 = LocalWorkspaceBackend::search_sync(
            &search_config,
            &search_roots,
            "lib",
            None,
            None,
            None,
            None,
            10,
            Duration::from_secs(10),
            start2,
            cache_ref,
            &RegexSymbolBackend,
        );

        let t2 = _r2.telemetry.as_ref().unwrap();
        assert!(t2.used_inventory);
        assert!(t2.inventory_fresh);
        assert!(!t2.cold_build);
    }

    #[test]
    fn test_git_inventory_includes_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::write(root.join("tracked.rs"), "fn tracked() {}").unwrap();
        fs::write(root.join("untracked.rs"), "fn untracked() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("tracked.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };

        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_some());
        let ri = ri.unwrap();
        assert!(ri.uses_git_backend);

        let paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"tracked.rs"));
        assert!(paths.contains(&"untracked.rs"));
        assert!(ri.untracked_count >= 1);
    }

    #[test]
    fn test_git_inventory_excludes_ignored_untracked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("debug.log"), "log data").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg(".gitignore")
            .arg("main.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: true,
            ..Default::default()
        };

        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_some());
        let ri = ri.unwrap();

        let paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"main.rs"));
        assert!(!paths.contains(&"debug.log"));
    }

    #[test]
    fn test_git_inventory_excludes_hidden_components() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/secret.rs"), "fn secret() {}").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg(".hidden/secret.rs")
            .arg("main.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            include_hidden: false,
            respect_gitignore: false,
            ..Default::default()
        };

        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_some());
        let ri = ri.unwrap();

        let paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"main.rs"));
        assert!(!paths.contains(&".hidden/secret.rs"));
    }

    #[test]
    fn test_git_inventory_excludes_skip_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target/debug.rs"), "fn debug() {}").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("target/debug.rs")
            .arg("main.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };

        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_some());
        let ri = ri.unwrap();

        let paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"main.rs"));
        assert!(!paths.iter().any(|p| p.starts_with("target/")));
    }

    #[test]
    fn test_nul_delimited_paths_with_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::create_dir_all(root.join("my folder")).unwrap();
        fs::write(root.join("my folder/file.rs"), "fn main() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("my folder/file.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };

        let ri = build_inventory_git(0, root, &config);
        assert!(ri.is_some());
        let ri = ri.unwrap();

        let paths: Vec<&str> = ri
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(
            paths.contains(&"my folder/file.rs"),
            "NUL-delimited path with spaces should be parsed correctly: {paths:?}",
        );
    }

    #[test]
    fn test_git_index_mtime_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .output()
            .unwrap();

        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("main.rs")
            .current_dir(root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(root)
            .output()
            .unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv1 = build_inventory(&config, &roots);
        assert!(!needs_rebuild(&inv1, &config, Duration::from_secs(3600)));

        std::thread::sleep(Duration::from_secs(1));

        fs::write(root.join("lib.rs"), "pub fn lib() {}").unwrap();
        std::process::Command::new("git")
            .arg("add")
            .arg("lib.rs")
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("add lib")
            .current_dir(root)
            .output()
            .unwrap();

        assert!(
            needs_rebuild(&inv1, &config, Duration::from_secs(3600)),
            "HEAD change should trigger rebuild"
        );
    }
}
