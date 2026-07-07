# Release Tightening Corrective Plan

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
- architecture/module deep-dive docs.

The shape is good. Remaining work is correctness alignment, documentation accuracy, and release-gate verification.

## Corrective Workstream 1: Fix IPv4 `192.0.0.0/24` Overblocking

### Problem

The new `is_blocked_v4` implementation appears to block more than the documented `192.0.0.0/24` range.

The current predicate includes a condition equivalent to:

```rust
(octet0 == 192 && o[1] == 0)
```

That blocks all of `192.0.0.0/16`, not only `192.0.0.0/24`. The docs and plan describe blocking `192.0.0.0/24`, plus separately blocking documentation ranges such as `192.0.2.0/24`.

This is conservative from an SSRF perspective, but it is broader than documented and may block legitimate globally routable addresses in `192.0.1.0/24` through `192.0.255.0/24`, depending on allocation. For release correctness, implementation and docs must match.

### Required change

In `src/fetch/limits.rs`, update the `192.0.0.0/24` check from the broad `/16` condition to an exact `/24` condition:

```rust
(octet0 == 192 && o[1] == 0 && o[2] == 0)
```

Keep the existing explicit check for `192.0.2.0/24` unless the code is refactored into a table. The result should block both:

- `192.0.0.1` as IETF protocol assignment range;
- `192.0.2.1` as TEST-NET-1 documentation range.

But it should not block:

- `192.0.3.1`, assuming no other policy intentionally blocks it.

### Recommended tests

Add tests to `tests/fetch_safety.rs` or the existing `src/fetch/limits.rs` test module:

```rust
#[tokio::test]
async fn g2c_ipv4_192_0_0_24_is_blocked_but_not_192_0_0_16() {
    let limits = FetchLimits {
        allow_private_network: false,
        allow_localhost: false,
        ..Default::default()
    };

    for url in ["http://192.0.0.1/", "http://192.0.2.1/"] {
        let req_url = url::Url::parse(url).unwrap();
        assert!(validate_fetch_target(&req_url, &limits).await.is_err());
    }

    let req_url = url::Url::parse("http://192.0.3.1/").unwrap();
    assert!(validate_fetch_target(&req_url, &limits).await.is_ok());
}
```

If maintainers intentionally want the more conservative `/16` block, update docs and tests to say that explicitly. Prefer exact documented behavior unless there is a concrete threat-model reason to broaden it.

### Acceptance criteria

- `192.0.0.1` remains blocked.
- `192.0.2.1` remains blocked.
- `192.0.3.1` is allowed by address policy by default.
- Docs and implementation agree on the range.
- No live network request is required for the test.

## Corrective Workstream 2: Remove or Use Dead IPv4 CIDR Helpers

### Problem

The SSRF pass added helper functions resembling:

```rust
#[allow(dead_code)]
fn ipv4_to_u32(...)

#[allow(dead_code)]
fn ipv4_in_cidr(...)
```

They are currently marked `#[allow(dead_code)]`. Dead private helpers are not a production bug, but they add noise in a security-sensitive predicate. In this part of the codebase, clarity matters more than speculative future utility.

### Preferred change

Either:

1. remove both unused helpers and keep the explicit octet checks; or
2. refactor `is_blocked_v4` to use `ipv4_in_cidr` for all CIDR checks, then remove `#[allow(dead_code)]`.

Prefer option 1 if the current octet-based predicate remains readable after the `/24` fix. Prefer option 2 if maintainers want a declarative table of CIDR blocks.

### Acceptance criteria

- No `#[allow(dead_code)]` is needed for private IPv4 helpers in `src/fetch/limits.rs`.
- The address-blocking predicate remains easy to audit.
- Existing blocked/allowed address tests still pass.

## Corrective Workstream 3: Expand Edge-Case Address Tests

### Problem

The new SSRF test coverage is strong, but this corrective pass should lock down the exact boundary cases that caused the overblocking issue. Tests should cover adjacent ranges, not just representative blocked addresses.

### Required tests

Add explicit tests for:

#### IPv4 exact boundaries

Blocked:

- `0.0.0.0`
- `0.255.255.255`
- `10.0.0.1`
- `100.64.0.1`
- `100.127.255.255`
- `127.0.0.1`
- `169.254.169.254`
- `172.16.0.1`
- `172.31.255.255`
- `192.0.0.1`
- `192.0.2.1`
- `192.88.99.1`
- `192.168.0.1`
- `198.18.0.1`
- `198.19.255.255`
- `198.51.100.1`
- `203.0.113.1`
- `224.0.0.1`
- `239.255.255.255`
- `240.0.0.1`
- `255.255.255.255`

Allowed by address policy:

- `1.1.1.1`
- `8.8.8.8`
- `100.128.0.1`
- `172.32.0.1`
- `192.0.3.1`
- `198.20.0.1`
- `223.255.255.255`

These allowed examples are address-policy tests only. They should not attempt real fetches.

#### IPv6 exact categories

Blocked:

- `::`
- `::1`
- `fc00::1`
- `fd00::1`
- `fe80::1`
- `ff00::1`
- `2001:db8::1`
- `2001:2::1`
- `2001::1`
- `2002::1`
- `::ffff:10.0.0.1`
- `::ffff:100.64.0.1`
- `::ffff:192.0.2.1`
- `::ffff:198.18.0.1`

Allowed by address policy:

- `2606:4700:4700::1111`

### Acceptance criteria

- Tests are deterministic and offline.
- Tests validate through the same public or crate-visible validation path used by fetch.
- Boundary behavior is documented by test names.

## Corrective Workstream 4: Documentation Accuracy Pass for Architecture Docs

### Problem

The new architecture docs are valuable, but at least one architecture excerpt describes SSRF protection in a narrower, older form: private IPs, localhost, and IPv6 loopback. The actual implementation now blocks a broader set of private, loopback, link-local, multicast, reserved, documentation, benchmarking, carrier-grade NAT, and IPv4-mapped IPv6 ranges.

Docs should not be weaker or stale in a security-sensitive section.

### Required changes

Audit and update:

- `docs/architecture/fetch.md`
- `docs/architecture/overview.md`
- `docs/architecture/codegg-contract.md`
- `docs/safety.md`
- `docs/config.md`
- README safety summary if needed

Ensure all references agree on:

- fetch is explicit single URL only;
- no crawling;
- no JavaScript execution;
- redirect targets are revalidated;
- DNS-resolved addresses are validated and pinned for the request attempt;
- blocked address classes include private, loopback, link-local, CGNAT, multicast, reserved, benchmarking, documentation, and IPv4-mapped IPv6 forms;
- `allow_private_network` and `allow_localhost` remain operator escape hatches and should stay disabled for general MCP exposure.

### Acceptance criteria

- No architecture doc uses the old shorthand as the complete policy.
- Deep docs and README summary agree with `docs/safety.md`.
- Documentation does not imply that fetch performs crawling or browser-like behavior.

## Corrective Workstream 5: Provider Health and Skip-Code Consistency Audit

### Problem

Provider skip codes and health views are now implemented. Before release, verify that the two state surfaces remain consistent and do not confuse configuration state with runtime health state.

### Required audit

Inspect:

- `src/core/provider.rs`
- `src/meta/provider_diagnostics.rs`
- `src/meta/adapter.rs`
- `src/mcp/tools.rs`
- `src/commands/providers.rs`
- docs mentioning `provider_status`, `skip_code`, `health`, or `health_views`

Check the following:

- non-routable due to config uses a skip code such as `disabled_by_user`, `missing_api_key`, `credential_env_missing`, etc.;
- non-routable due to cooldown uses `cooldown_active` only if cooldown actually affects routing;
- explicitly requested providers are documented as not skipped for cooldown if that remains the implementation;
- `health.status = Unknown` is used for no observations and not for misconfiguration;
- bounded provider error messages are not duplicated in multiple response fields unnecessarily;
- CLI JSON and MCP output use the same serialized code names.

### Tests to add if gaps are found

- provider disabled + no health observations -> config skip code with `health.status = Unknown`;
- provider configured + three failures -> `health.status = Cooldown` and skip/routable behavior matches docs;
- explicitly requested cooled-down provider behavior is tested or documented;
- CLI JSON and MCP provider-status fixture contain matching skip-code names.

### Acceptance criteria

- Configuration state and health state remain distinct.
- Skip-code docs match serialized enum names.
- Provider health behavior is clear to codegg and other MCP clients.

## Corrective Workstream 6: Release-Gate Verification on Exact Corrective Commit

### Problem

The repository’s release docs require direct verification on the exact release commit. The previous review could not see GitHub Actions runs or combined statuses for the latest head through the connector. That may be connector/API visibility, but it still means release readiness cannot be asserted from commit messages alone.

### Required verification

After the corrective code/docs changes land, run or confirm:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --all-features --test fetch_safety
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

If `make check` covers the local gate, run it as well, but do not assume it covers docs build and publish dry-run unless the Makefile explicitly does.

### GitHub CI expectations

Confirm on the exact corrective commit SHA:

- formatter job passes;
- clippy all-targets/all-features job passes;
- all-features test job passes;
- no-default-features test job passes;
- mock feature test job passes;
- pdf feature test job passes if configured;
- docs-contract job passes;
- release build job passes;
- publish dry-run job passes;
- rustdoc warnings-as-errors job passes.

If GitHub Actions are not running for pushes to main, fix workflow triggering or document why release verification is local-only for this pass.

### Acceptance criteria

- Exact corrective commit has a recorded verification result.
- CI and local gate status are not inferred from commit messages.
- Any warnings are either fixed or explicitly documented as non-blocking.

## Recommended Implementation Order

1. Patch the IPv4 `/24` predicate and remove/refactor dead helpers.
2. Add boundary tests for IPv4 and IPv6 address policy.
3. Run targeted fetch tests.
4. Update architecture/safety docs for address-policy accuracy.
5. Audit provider skip-code/health consistency and add tests only where gaps are found.
6. Run the full release gate and verify GitHub CI on the final corrective commit.

## Files Likely to Change

Expected code/test files:

- `src/fetch/limits.rs`
- `tests/fetch_safety.rs`
- possibly `tests/integration.rs`
- possibly `tests/schema_identity_registry.rs`

Expected docs files:

- `docs/architecture/fetch.md`
- `docs/architecture/overview.md`
- `docs/architecture/codegg-contract.md`
- `docs/safety.md`
- `docs/config.md`
- maybe `README.md`

Avoid touching unrelated provider engine implementations unless the provider health/skip-code audit finds a concrete inconsistency.

## Non-Goals

- No new providers.
- No new MCP tools.
- No live probe implementation for `provider_status.probe`.
- No persistent provider-health database.
- No browser/JS/crawler fetch expansion.
- No public API reshaping beyond corrective consistency fixes.

## Definition of Done

This corrective pass is complete when:

- the `192.0.0.0/24` implementation matches the documented range;
- adjacent IPv4 and IPv6 boundary tests lock down the address policy;
- unused dead-code helpers are removed or used;
- architecture/safety docs accurately describe the current fetch policy;
- provider health and skip-code docs/tests remain consistent;
- `make check` or equivalent local release gate passes;
- GitHub CI is verified on the exact corrective commit, or the reason CI is unavailable is explicitly documented for handoff.
