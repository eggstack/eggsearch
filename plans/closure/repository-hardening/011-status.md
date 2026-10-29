# M011 macOS Symlink Containment and Test Portability — Closure Status

Status: closed

Source plan: `plans/implementation/repository-hardening/011-macos-symlink-containment-and-test-portability.md`

Roadmap: `plans/subsystems/repository-hardening-roadmap.md`

Corrective candidate: `99316f31caf37ad2165cd5e822316942faf20c6f`

Original finding: deterministic retrieval M006 candidate
`10b06ab76434983cd9942e81bf2090fc9d6e31cc`; closure
`plans/closure/deterministic-retrieval-quality/006-status.md`.

## Findings and implementation

The M006 macOS qualification exposed that `safe_open_relative()` fell through
to ordinary `openat()` when `follow_symlinks=true` on non-Linux Unix. Since
that path omitted `O_NOFOLLOW` and did not have a beneath-root resolver, an
outside-root symlink was followed despite the documented
`SafeSymlinkFollowingUnsupported` contract.

Candidate `99316f3` now returns `SafeSymlinkFollowingUnsupported` on
non-Linux Unix before opening any path when symlink following is requested.
When following is disabled, descriptor-relative component opens retain
`O_NOFOLLOW`. Linux keeps its `openat2(RESOLVE_BENEATH)` follow behavior.
The host-scoped test compares roots against their canonical filesystem paths,
matching the canonicalization performed by `local_workspace_view()` while
preserving distinct-root and cache-reuse assertions. The in-root-follow test
and other tests that require following a file are Linux-only; non-Linux Unix
has explicit typed unsupported assertions for both a regular file and an
outside-root symlink. `architecture/hardening.md` now matches the code and
existing `architecture/local-workspace.md` / `docs/safety.md` contract.

## Requirement-to-evidence

| Requirement | Evidence | Outcome |
|---|---|---|
| Outside-root symlink cannot be followed on macOS | `safe_open_follow_symlink_is_unsupported` asserts the typed error on an outside-root symlink; repo-mapper regression passes | Pass; platform fails closed |
| Non-Linux follow mode has explicit contract | The same test also asserts `SafeSymlinkFollowingUnsupported` for a regular in-root file when following is configured | Pass; behavior does not depend on symlink target |
| Linux in-root follow remains available | The Linux `openat2` code path and Linux-only follow tests are unchanged | Not rerun on this macOS host; code path is outside the corrected cfg branch |
| macOS root aliases do not break distinct-root scope behavior | `host_scopes_authenticate_and_cache_distinct_roots` compares canonical roots and retains pointer identity, auth, invalidation, and removed-root assertions | Pass |
| Contributor documentation agrees with runtime contract | `architecture/hardening.md` corrected; local-workspace and safety docs already stated unsupported behavior | Pass |
| Corrected exact candidate passes canonical macOS gate | `make check` on `99316f3` | Pass; 3,486 all-feature tests passed |

## Verification

Host: macOS 26.6.2, x86_64.

- `cargo fmt --check`: passed.
- `cargo test --locked --all-features --lib meta::repo_mapper::tests::structure_file_reader_rejects_symlink_escape_even_when_following_is_enabled`: 1 passed.
- `cargo test --locked --all-features --lib mcp::state::tests::host_scopes_authenticate_and_cache_distinct_roots`: 1 passed.
- `cargo test --locked --all-features --test property_local_fs_extended`: 29 passed.
- `cargo test --locked --no-default-features --test property_local_fs_extended`: 29 passed.
- `cargo test --locked --features pdf --test property_local_fs_extended`: 29 passed.
- `cargo test --locked --features browser --test property_local_fs_extended`: 29 passed.
- `cargo test --locked --features 'pdf browser' --test property_local_fs_extended`: 29 passed.
- `make check`: passed, including fmt, all-target/all-feature clippy, no-default check, 3,486 all-feature tests, hygiene, dependency policy, and packaging check.
- `make docs-check`: passed.

## Compatibility, security, and disposition

The fix removes accidental symlink-following behavior on non-Linux Unix and
restores the already documented unsupported result. It does not change public
MCP/CLI contracts, Linux behavior, or ranking/provider behavior. Configurations
requesting symlink following on macOS now fail closed; this is the documented
platform limitation. No remaining high-severity finding from M011 is open.

Close M011. Its required condition for deterministic retrieval M006 is
discharged by this exact-candidate evidence; M006 is fully closed in its
closure record and registry references.
