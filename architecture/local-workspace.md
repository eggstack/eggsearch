# Local Workspace Deep Dive

**Path:** `src/core/local.rs`, `src/meta/local_backend.rs`, `src/meta/local_inventory.rs`, `src/meta/local_inventory_cache.rs`, `src/meta/local_ignore.rs`, `src/meta/safe_open.rs`
**Purpose:** Filesystem-based workspace search with cached inventory, git-aware fast path, race-resistant file opening, and ignore-rule support.

---

## Overview

The local workspace subsystem provides search over local files without cloning or indexing. It uses a cached file inventory (auto-built on first search), git-aware fast path (`git ls-files`), bounded file walking, and race-resistant file opening via `openat2`.

---

## Configuration (`src/core/local.rs`)

```toml
[local]
enabled = true
roots = ["/path/to/workspace"]
max_file_bytes = 1048576      # 1MB
max_indexed_files = 10000
include_hidden = false
respect_gitignore = true
follow_symlinks = false
```

### Path Policy

Centralized in `local.rs`:
- **Hidden files**: rejected unless `include_hidden = true`
- **SKIP_DIRS**: `target/`, `node_modules/`, `.git/`, `__pycache__/`, etc.
- **Binary extensions**: `.exe`, `.dll`, `.so`, `.dylib`, `.bin`, etc.
- **Symlinks**: rejected unless `follow_symlinks = true`
- **Size**: `max_file_bytes` cap per file

---

## Git Worktree Discovery (`src/meta/local_inventory.rs`)

### Identity Resolution

Reads `.git/config` directly (no `git` subprocess for config) to normalize remote URLs into `NormalizedRepoId`:
- Supports HTTPS, SSH, SCP, git protocols
- Host alias resolution (github.com → github, gitlab.com → gitlab)

### Worktree State

Detects:
- Current branch
- HEAD commit SHA
- Dirty state via `git status --porcelain`
- Untracked/ignored file counts
- Manifest files (`Cargo.toml`, `package.json`, etc.)

### Workspace ID

Deterministic `workspace_id` via FNV-1a hash of the workspace root path. Stable across calls unless configuration changes.

---

## File Inventory Cache (`src/meta/local_inventory_cache.rs`)

### Cache Structure

```
WorkspaceInventory
  ├── roots: Vec<WorkspaceRootInventory>
  │   └── WorkspaceRootInventory
  │       ├── root: PathBuf
  │       ├── entries: Vec<InventoryEntry>
  │       ├── index_mtime: SystemTime
  │       ├── status_hash: Option<String>  (XXH3 of git status output)
  │       └── build_duration: Duration
  └── built_at: Instant
```

### Entry Fields

`InventoryEntry`: path, relative_path, root_index, size, language, mtime, xxh3_hash

### Build Strategy

1. **Git fast path**: `git ls-files -z --cached --others --exclude-standard`
2. **Fallback**: native directory walking via `walkdir`
3. **Inventory auto-build**: triggered on first search (cache miss)
4. **Rebuild detection**: status_hash change, index_mtime change, or TTL expiry (300s)

### Freshness Confidence

| Age | Status Hash | Confidence |
|-----|-------------|------------|
| < 30s | Any | `High` |
| < 300s | Unchanged | `Medium` |
| >= 300s | Any | `Low` |

### Bounded Command Runner

`run_bounded_command()` enforces:
- **Timeout**: 5s
- **stdout cap**: 16MB
- **stderr cap**: 64KB
- **Concurrent drainage**: stdout thread + stderr main thread
- **Process groups**: `setsid()` + kill on timeout
- **Cap breach**: immediate process group termination via `ProcessTerminationController`

`CommandTermination` enum: `Exited`, `TimedOut`, `StdoutLimitExceeded`, `StderrLimitExceeded`, `SpawnFailed`, `Signaled`

### Validation

`validate_entry()` rejects:
- Deleted files (mtime mismatch)
- Oversized files (> max_file_bytes)
- Symlinks (when follow_symlinks = false)

---

## Local Backend (`src/meta/local_backend.rs`)

### Search Flow

```
1. Ensure inventory is built (auto-build on cache miss)
2. Candidate filtering by query terms
3. Bounded content reads (max_file_bytes)
4. Scoring: path match + text match + language match + symbol match
5. SourceCard conversion with trust = LocalTrusted
6. Telemetry: backend used, inventory age, files considered/read, bytes read
```

### Symbol Backend

`SymbolBackend` trait with `RegexSymbolBackend` implementation:
- Compiles symbol patterns for Rust, Python, JavaScript, Go
- Matches against file content with line-level precision
- Bounded scan lines per file

### File Classification

`is_generated()`, `is_vendor()`, `is_test()`, `is_example()`, `is_config()`, `is_lockfile()` — path-based heuristics for result metadata.

---

## Ignore Rules (`src/meta/local_ignore.rs`)

Minimal `.gitignore` matcher via `IgnoreStack`:
- Comments, negation (`!`)
- Anchored patterns (`/`-prefix)
- Directory-only patterns (`/`-suffix)
- Wildcards: `*`, `**`, `?`
- Nested `.gitignore` support
- Layered evaluation (parent → child)

---

## Race-Resistant File Opening (`src/meta/safe_open.rs`)

### `safe_open_relative()`

Component-wise path walking using descriptor-relative open:
- **Linux**: `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`
- **Fallback**: `openat` with `O_NOFOLLOW`
- **Follow mode** (Linux): `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` (allows symlinks within bounds)
- **Non-Linux**: `SafeSymlinkFollowingUnsupported`

### Verification

Final file descriptor checked via `fstat`:
- Regular file type
- Size within limits

### Security Properties

- Eliminates TOCTOU races between validation and open
- Rejects `..` traversal
- Validates relative paths only
- No symlink escape from canonical root

---

## Source Cards

Local results produce `SourceCard` with:
- `trust`: `LocalTrusted`
- `local_repo_match`: root_path, remote_host/owner/repo, branch, commit, dirty_state
- `file_classification`: is_source, is_test, is_config, is_documentation, is_generated, language, size_bytes
- `workspace_id`: deterministic workspace identifier

---

**Back to:** [overview.md](overview.md)
