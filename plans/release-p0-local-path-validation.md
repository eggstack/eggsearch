# P0 Release Plan: Component-Aware Local Workspace Path Validation

Status: handoff plan
Priority: P0 release blocker
Area: local workspace search/fetch safety and correctness

## Problem

`validate_local_fetch_path` currently rejects any requested local workspace path containing the substring `..`. That is conservative but too broad. It correctly blocks obvious traversal attempts such as `../secret`, but it can also reject legitimate root-relative filenames that contain two dots as part of a normal filename, such as `foo..bar.rs`, `schema..generated.ts`, or documentation paths with double-dot naming.

The current validation also only checks `requested_relative_path.starts_with('/')` for absolute paths. This is sufficient for Unix absolute paths but should be made explicitly component-aware and cross-platform for Windows-style absolute or prefix paths, even if the primary target is macOS/Linux.

The goal is to preserve the existing safety properties while avoiding false positives.

## Relevant Code

Primary files:

- `src/core/local.rs`
- local workspace tests in `src/core/local.rs`
- any integration tests that exercise `repo_fetch` or local workspace fetch behavior

Function of interest:

- `validate_local_fetch_path(root, requested_relative_path, cfg)`

Current safety checks to preserve:

- Empty path rejection.
- Absolute path rejection.
- Parent-directory traversal rejection.
- Binary extension rejection.
- Existence check.
- Symlink policy enforcement.
- Canonical root containment.
- Regular-file requirement.

## Implementation Plan

### 1. Replace substring traversal detection with component-aware detection

Use `std::path::Path::new(requested_relative_path).components()` and reject only `Component::ParentDir`.

Recommended helper:

```rust
fn has_parent_dir_component(path: &Path) -> bool {
    path.components().any(|component| matches!(component, std::path::Component::ParentDir))
}
```

Then in validation:

```rust
let requested_path = Path::new(requested_relative_path);
if has_parent_dir_component(requested_path) {
    return Err(LocalFetchPathError::PathTraversal);
}
```

Do not reject normal components whose file names merely contain two dots.

### 2. Harden absolute/prefix detection

Use component-aware absolute detection instead of only checking a leading slash. Reject:

- `requested_path.is_absolute()`
- `Component::RootDir`
- `Component::Prefix(_)` on Windows-style paths

Even on Unix, a path such as `C:\Users\...` may be treated as a normal component, but having the explicit helper improves portability under Windows CI or downstream users.

Recommended helper:

```rust
fn is_absolute_or_prefixed(path: &Path) -> bool {
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Prefix(_)
            )
        })
}
```

Use this in place of `starts_with('/')`.

### 3. Keep canonical containment as the authoritative boundary

The final `canonical.starts_with(root_canonical)` check must remain. Component filtering is an early rejection and clearer error reporting; canonical containment remains the authoritative escape-prevention check after symlink resolution.

### 4. Add regression tests

Add or update tests in `src/core/local.rs`.

Required tests:

- Rejects `../secret.rs`.
- Rejects `a/../../secret.rs`.
- Rejects `/absolute/path.rs` on Unix.
- Rejects a symlink escape when `follow_symlinks = true` and target resolves outside the root.
- Allows a legitimate file named `foo..bar.rs`.
- Allows a legitimate nested file named `src/generated..schema.rs`.
- Keeps binary extension rejection behavior.
- Keeps symlink rejection behavior when `follow_symlinks = false`.

Test structure should use `tempfile` rather than relying on repository files.

Example skeleton:

```rust
#[test]
fn local_fetch_allows_double_dot_in_filename() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("foo..bar.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let cfg = LocalConfig::default();
    let resolved = validate_local_fetch_path(tmp.path(), "foo..bar.rs", &cfg).unwrap();
    assert_eq!(resolved, file.canonicalize().unwrap());
}
```

### 5. Audit search path matching if it reuses similar substring logic

Search for:

- `contains("..")`
- `starts_with('/')`
- `PathTraversal`
- `EscapesRoot`

If equivalent local workspace path validation exists elsewhere, route it through the same helper or document why it differs.

### 6. Avoid changing public response schemas

This is a correctness/safety patch. It should not change MCP schema output or tool argument names.

## Acceptance Criteria

The implementation is complete when:

- Legitimate filenames containing `..` are accepted if they remain inside the configured root.
- Actual `..` path components are rejected before filesystem access.
- Canonical containment is still enforced after symlink resolution.
- Existing local workspace safety tests still pass.
- New regression tests cover double-dot filenames and parent-directory traversal.
- `cargo test --all-features` passes.
- `cargo test --no-default-features` passes.
- `cargo clippy --all-features -- -D warnings` passes.

## Manual Verification

Create a temporary local workspace with:

```text
workspace/
  foo..bar.rs
  src/generated..schema.rs
  safe.rs
```

Enable local workspace search and verify `repo_fetch` can fetch `foo..bar.rs` and `src/generated..schema.rs` by relative path.

Then verify these are rejected:

```text
../outside.rs
src/../../outside.rs
/absolute/outside.rs
```

## Risk Notes

Do not weaken containment checks. The fix is to reduce false positives, not to permit ambiguous paths. If in doubt, keep the canonical containment check strict and prefer explicit test coverage over ad hoc path string manipulation.
