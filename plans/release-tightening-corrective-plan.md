# Release Tightening Corrective Plan

## Status: COMPLETE

All 6 corrective workstreams have been implemented and verified. Committed as
`1bc1302` on `main`, pushed to `origin/main`.

## Purpose

This plan closes the remaining issues found after implementing the first four release-tightening milestones. The prior passes substantially improved the repository: fetch SSRF hardening, provider skip-code diagnostics, raw-text response schema handling, and provider health observability are now implemented with code, tests, and documentation.

This corrective pass should be small and surgical. Do not add new providers, new MCP tools, or new feature work. The goal is to remove the remaining release-risk details before treating the first release-tightening tranche as closed.

## Current State

Post-plan `main` includes implementation commits for:

- comprehensive fetch address-blocking policy;
- redirect-target revalidation tests;
- code-host rewrite SSRF tests;
- machine-readable provider skip codes;
- provider-status and CLI skip-code output;
- raw-text metadata and MCP omission tests;
- outline title Tier-1 sanitization;
- provider health views, cooldown docs, panic classification, and bounded health error messages;
- architecture/module deep-dive docs;
- **IPv4 192.0.0.0/24 overblocking fix;**
- **dead CIDR helper removal;**
- **exact IPv4/IPv6 boundary tests;**
- **SSRF documentation accuracy update;**
- **rustdoc private-item link fix.**

The shape is good. Remaining work is correctness alignment, documentation accuracy, and release-gate verification.

## Corrective Workstream 1: Fix IPv4 `192.0.0.0/24` Overblocking

### Problem

The new `is_blocked_v4` implementation appeared to block more than the documented `192.0.0.0/24` range. The predicate included `(octet0 == 192 && o[1] == 0)` which blocked all of `192.0.0.0/16`.

### Solution

Changed to `(octet0 == 192 && o[1] == 0 && o[2] == 0)` in `src/fetch/limits.rs:316`. Now blocks exactly `192.0.0.0/24`, plus separately `192.0.2.0/24` (TEST-NET-1).

### Tests

Added `is_blocked_v4_192_0_0_24_exact` unit test and integration tests in sections N and O of `tests/fetch_safety.rs`.

## Corrective Workstream 2: Remove Dead IPv4 CIDR Helpers

### Solution

Removed `ipv4_to_u32()` and `ipv4_in_cidr()` functions (were `#[allow(dead_code)]`) from `src/fetch/limits.rs:372-383`.

## Corrective Workstream 3: Expand Edge-Case Address Tests

### Solution

Added section N (n1-n3) for IPv4 exact boundaries and section O (o1-o2) for IPv6 exact categories in `tests/fetch_safety.rs`. Tests cover blocked and allowed addresses at range boundaries.

## Corrective Workstream 4: Documentation Accuracy Pass

### Solution

- `docs/safety.md` already correctly describes `192.0.0.0/24` — no change needed.
- `docs/architecture/fetch.md` updated to reference RFC 1918/6890, loopback, link-local, multicast, documentation ranges, and IPv6 equivalents with cross-link to `safety.md`.

## Corrective Workstream 5: Provider Health/Skip-Code Consistency Audit

### Result

No inconsistencies found. All 13 `ProviderSkipCode` variants use `#[serde(rename_all = "snake_case")]`. `provider_skip_code()` derives correctly. `ProviderHealthStatus` is consistent with skip codes. Configuration state and runtime health state are properly distinct.

## Corrective Workstream 6: Release-Gate Verification

### Result

All CI gates pass:
- `cargo fmt --check` ✓
- `cargo clippy --all-targets --all-features -- -D warnings` ✓
- `cargo test --all-features`: 3475 passed, 5 ignored ✓
- `make schema-corpus` (6 regression binaries) ✓
- `make docs-tests` (3 doc contract tests) ✓
- `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` ✓
- `cargo publish --dry-run --allow-dirty` ✓

## Files Changed

| File | Changes |
|------|---------|
| `src/fetch/limits.rs` | Fixed /24 predicate, removed dead helpers, added unit test |
| `src/meta/provider_diagnostics.rs` | Fixed rustdoc private-item link |
| `docs/architecture/fetch.md` | Updated SSRF description with cross-link |
| `tests/fetch_safety.rs` | Added sections N (IPv4 boundaries) and O (IPv6 boundaries) |

## Definition of Done

This corrective pass is complete when:

- the `192.0.0.0/24` implementation matches the documented range;
- adjacent IPv4 and IPv6 boundary tests lock down the address policy;
- unused dead-code helpers are removed or used;
- architecture/safety docs accurately describe the current fetch policy;
- provider health and skip-code docs/tests remain consistent;
- `make check` or equivalent local release gate passes;
- GitHub CI is verified on the exact corrective commit, or the reason CI is unavailable is explicitly documented for handoff.
