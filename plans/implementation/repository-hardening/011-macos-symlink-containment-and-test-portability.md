# M011 — macOS Symlink Containment and Test Portability Corrective

Status: ready
Source roadmap: `plans/subsystems/repository-hardening-roadmap.md`
Corrects: deterministic-retrieval-quality M006, conditionally closed at
`plans/closure/deterministic-retrieval-quality/006-status.md`
Primary class: security and portability corrective
Baseline: `10b06ab76434983cd9942e81bf2090fc9d6e31cc`

## Objective

Restore the documented local-workspace containment behavior when symlink
following is enabled on macOS, make the affected root-path test portable to
macOS temporary-directory aliases, and produce a green exact-candidate
canonical gate on macOS.

## Findings

1. On the M006 macOS x86_64 host,
   `meta::repo_mapper::tests::structure_file_reader_rejects_symlink_escape_even_when_following_is_enabled`
   failed because `read_structure_file` followed a symlink outside the
   workspace root. The non-Linux Unix `openat` path does not provide the
   Linux `RESOLVE_BENEATH` containment guarantee.
2. `mcp::state::tests::host_scopes_authenticate_and_cache_distinct_roots`
   failed because the test treated `/private/var/...` and `/var/...` as
   distinct roots on macOS even though they are filesystem aliases.

The first finding is a safety-boundary defect. M006 verification did not
surface either issue until it ran on a macOS host; earlier qualification ran
on Linux.

## Scope and invariants

- Preserve the documented rule that non-Linux Unix with
  `follow_symlinks=true` fails closed as `SafeSymlinkFollowingUnsupported`
  unless a platform-correct containment mechanism is implemented and proven.
- Do not weaken root containment, convert an escape into an accepted file, or
  silently fall back to an unsafe open path.
- Make the root identity test compare canonical filesystem identity or
  otherwise account for macOS temporary-root aliases while still detecting
  genuinely distinct roots.
- Do not alter retrieval ranking, provider routing, MCP contracts, or public
  feature defaults.

## Required work and evidence

1. Trace the macOS `safe_open` implementation and callers. Select either a
   correct fail-closed unsupported behavior or a platform-native constrained
   open strategy; document why the chosen mechanism prevents a symlink escape.
2. Add regression coverage for an in-root file, an in-root symlink if the
   platform contract supports it, and a symlink escape with following enabled.
   The escape case must fail on macOS and other supported Unix targets.
3. Update the root-scoping test to use filesystem identity semantics and keep
   assertions that distinct roots remain isolated.
4. Run focused tests for safe-open/repo mapping and MCP state on macOS, then
   `cargo fmt --check`, clippy, all feature combinations, `make check`,
   `make docs-check`, and `make packaging-check` on the exact candidate.
5. Record the tested macOS version/architecture and exact commit in a separate
   corrective closure record. Reconcile this plan, repository-hardening
   roadmap, and registry in that closure commit; then update M006 from
   conditional to closed by reference to the corrective evidence.

## Acceptance criteria

- The outside-root symlink is rejected when symlink following is requested,
  or the API returns the explicit unsupported result required by the existing
  platform contract.
- Root-scoped state tests pass with macOS path aliases and still distinguish
  separate roots.
- The exact corrected candidate passes focused tests and the canonical
  macOS `make check` gate.
- No public retrieval behavior or security invariant regresses.

## Stop conditions

Stop if a safe macOS containment strategy cannot be demonstrated. Preserve
fail-closed behavior and report the platform as unsupported rather than
weakening the boundary.
