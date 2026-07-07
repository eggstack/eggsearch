# Milestone 1 Plan: Fetch and SSRF Hardening Closure

## Objective

Harden `web_fetch`, `batch_fetch`, and fetch-backed repo/evidence workflows against SSRF by making address-blocking policy explicit, comprehensive, test-backed, and documented.

This milestone should be completed before release. The fetch path is already substantially hardened, but the remaining special-use address coverage gap is significant enough to treat as a release-blocking closure item.

## Scope

In scope:

- direct URL validation;
- code-host URL rewrite validation;
- redirect target validation;
- DNS resolution validation;
- address pinning between validation and outbound request;
- IPv4 special-use blocking;
- IPv6 special-use blocking;
- IPv4-mapped IPv6 blocking;
- fetch safety tests;
- safety documentation updates.

Out of scope:

- JavaScript rendering;
- crawling or recursive fetch;
- persistent URL reputation storage;
- per-domain allowlists beyond current config knobs;
- browser automation;
- proxy support;
- live network smoke tests as release blockers.

## Relevant Code Areas

Primary files to inspect and modify:

- `src/fetch/limits.rs`
- `src/fetch/client.rs`
- `src/core/config.rs`
- `src/core/fetch.rs`
- `tests/fetch_safety.rs`
- any existing fetch-related integration tests
- `docs/safety.md`
- `docs/config.md`
- `docs/release.md` only if release commands change, which should not be necessary

The primary implementation target should be `src/fetch/limits.rs`, especially:

- `validate_url`;
- `validate_fetch_target`;
- `validate_fetch_target_with_resolved_addrs`;
- `is_blocked_address`;
- `is_blocked_v4`;
- `is_blocked_v6`;
- `ipv4_mapped_from_v6`.

## Current Problem Statement

The current fetch implementation blocks obvious localhost/private network targets by default and validates DNS resolution before connecting. That is correct. However, the IPv4 policy is narrower than a production SSRF policy should be. In particular, the comment around `is_blocked_v4` claims coverage for RFC1918 plus carrier-grade NAT `100.64/10`, but the implementation uses `Ipv4Addr::is_private`, which does not cover `100.64.0.0/10`.

The policy should not rely on a reader knowing exactly what the standard-library helpers do. For agent-facing fetch safety, the code should say exactly which ranges are blocked and tests should lock that behavior down.

## Design Requirements

### 1. Use explicit special-use range checks

Replace the current broad helper-only IPv4 logic with explicit range checks. It is acceptable to retain helper methods as part of the implementation, but they must not be the only policy. Define small helper functions for CIDR-like range checks using integer conversion or octet comparisons.

Recommended shape:

```rust
fn ipv4_to_u32(v4: Ipv4Addr) -> u32 { ... }
fn ipv4_in_cidr(v4: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool { ... }
fn is_blocked_v4(v4: Ipv4Addr) -> bool { ... }
```

Avoid adding a new dependency for CIDR parsing. This repository should keep the hardening pass stdlib/toolchain-oriented.

### 2. Block non-global and special-use IPv4 ranges by default

At minimum, block:

- `0.0.0.0/8` — current network / this host;
- `10.0.0.0/8` — RFC1918;
- `100.64.0.0/10` — carrier-grade NAT;
- `127.0.0.0/8` — loopback;
- `169.254.0.0/16` — link-local;
- `172.16.0.0/12` — RFC1918;
- `192.0.0.0/24` — IETF protocol assignments;
- `192.0.2.0/24` — documentation/test;
- `192.88.99.0/24` — deprecated 6to4 relay anycast, if treating non-global conservatively;
- `192.168.0.0/16` — RFC1918;
- `198.18.0.0/15` — benchmarking;
- `198.51.100.0/24` — documentation/test;
- `203.0.113.0/24` — documentation/test;
- `224.0.0.0/4` — multicast;
- `240.0.0.0/4` — reserved;
- `255.255.255.255/32` — broadcast.

Decide whether to block `192.0.0.9/32` and `192.0.0.10/32` exceptions as part of `192.0.0.0/24`. For a fetch SSRF defense, conservative blocking of the entire range is acceptable unless a concrete use case requires those exceptions. Document the choice.

### 3. Clarify `allow_localhost` versus `allow_private_network`

Current config has separate `allow_private_network` and `allow_localhost` flags. Preserve the separation.

Recommended semantics:

- `allow_localhost = false` blocks loopback and localhost-name targets even if `allow_private_network = true`, unless the implementation intentionally treats localhost as part of private network. Prefer separate blocking.
- `allow_private_network = false` blocks non-global private/special-use ranges other than localhost.
- Both must be true to fetch localhost and broad private/special-use targets.

If the existing behavior differs, either preserve existing behavior and document it clearly, or change it with regression tests. Do not leave it implicit.

### 4. Extend IPv6 policy

The current IPv6 policy blocks loopback, unspecified, unique-local, link-local, and IPv4-mapped IPv6 private equivalents. Extend or at least verify coverage for:

- `::/128` unspecified;
- `::1/128` loopback;
- `fc00::/7` unique-local;
- `fe80::/10` link-local;
- `ff00::/8` multicast;
- `2001:db8::/32` documentation;
- IPv4-mapped IPv6, including mapped versions of all blocked IPv4 ranges.

A conservative SSRF policy can block documentation and multicast ranges without affecting normal fetch usage.

### 5. Keep DNS validation and address pinning intact

`validate_fetch_target_with_resolved_addrs` resolves the host and returns the validated socket addresses. `FetchClient::client_for_url` then uses `resolve_to_addrs` so the outbound request uses the validated address set. Preserve this behavior.

Add tests that would fail if validation and connection resolution drift apart where practical. Full DNS-rebinding simulation is hard in unit tests, but the implementation should remain structured so the validated address list is the one passed into the client builder.

### 6. Validate all target transformations

The fetch client currently validates the initial URL, then may rewrite code-host URLs to raw URLs, then validates the raw URL. Preserve and test this.

Test target categories:

- initial URL is blocked;
- raw URL rewrite target is blocked;
- redirect target is blocked;
- DNS-resolved target is blocked.

## Implementation Steps

### Step 1: Add explicit address-range helpers

In `src/fetch/limits.rs`, add helpers for IPv4 range checks. Keep them private unless tests need module-level access. If direct unit tests need access, test through public validation functions where possible.

Example implementation direction:

```rust
fn ipv4_to_u32(v4: Ipv4Addr) -> u32 {
    u32::from_be_bytes(v4.octets())
}

fn ipv4_in_cidr(v4: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    (ipv4_to_u32(v4) & mask) == (ipv4_to_u32(network) & mask)
}
```

Ensure `prefix <= 32`. Since callers are static internal calls, a `debug_assert!(prefix <= 32)` is sufficient, but avoid panicking in release if a future caller makes a mistake.

### Step 2: Replace `is_blocked_v4`

Implement `is_blocked_v4` as explicit OR checks. Keep the implementation readable rather than clever. Add comments for each range.

Do not rely exclusively on `Ipv4Addr::is_private` or `is_loopback`; these helpers can remain only if the explicit ranges are present and tests cover them.

### Step 3: Extend `is_blocked_v6`

Add explicit checks for multicast and documentation ranges. Verify IPv4-mapped IPv6 calls `is_blocked_v4` so the full IPv4 denylist applies to mapped addresses.

Recommended helper:

```rust
fn ipv6_in_prefix(v6: Ipv6Addr, network_segments: [u16; 8], prefix: u8) -> bool { ... }
```

Given the small number of IPv6 ranges, explicit segment checks may be simpler:

- `ff00::/8`: `(segments[0] & 0xff00) == 0xff00`
- `2001:db8::/32`: `segments[0] == 0x2001 && segments[1] == 0x0db8`

### Step 4: Add unit tests for literal direct URLs

Add tests in `src/fetch/limits.rs` or `tests/fetch_safety.rs` for direct literal URLs. Cover both `http` and `https` only if the behavior is scheme-independent; one scheme is enough for most range tests.

Minimum blocked IPv4 examples:

- `http://0.0.0.0/`
- `http://10.0.0.1/`
- `http://100.64.0.1/`
- `http://127.0.0.1/`
- `http://169.254.1.1/`
- `http://172.16.0.1/`
- `http://192.168.0.1/`
- `http://192.0.2.1/`
- `http://198.18.0.1/`
- `http://198.51.100.1/`
- `http://203.0.113.1/`
- `http://224.0.0.1/`
- `http://240.0.0.1/`
- `http://255.255.255.255/`

Minimum allowed public examples:

- `http://1.1.1.1/`
- `http://8.8.8.8/`

These tests should call the validation layer, not perform outbound network requests.

### Step 5: Add IPv6 tests

Minimum blocked IPv6 examples:

- `http://[::]/`
- `http://[::1]/`
- `http://[fc00::1]/`
- `http://[fd00::1]/`
- `http://[fe80::1]/`
- `http://[ff00::1]/`
- `http://[2001:db8::1]/`
- `http://[::ffff:127.0.0.1]/`
- `http://[::ffff:10.0.0.1]/`
- `http://[::ffff:100.64.0.1]/`
- `http://[::ffff:198.18.0.1]/`

Minimum allowed public example:

- `http://[2606:4700:4700::1111]/`

### Step 6: Add redirect tests

Use `httpmock` to serve a redirect whose `Location` points to a blocked target. Verify the error is a redirect-target-blocked error, not a successful fetch.

Test cases:

- redirect to `http://127.0.0.1/`;
- redirect to `http://100.64.0.1/`;
- relative redirect remains allowed when it stays on the same safe host;
- redirect chain limit still behaves as before.

### Step 7: Add DNS-resolution tests where feasible

For DNS-resolved blocked targets, direct reliable unit tests are harder without controlling DNS. At minimum, test the pure address predicate and the literal URL path. If the code already has tests around `validate_fetch_target_with_resolved_addrs`, extend them.

Do not introduce flaky live DNS tests. This must remain offline-deterministic.

### Step 8: Add raw code-host rewrite safety test if practical

If code-host fetch target resolution can be triggered with a synthetic URL and raw URL can be made to point to a blocked address, add a regression test. If the resolver only emits known public raw hosts, document that initial and raw URL validation still both occur and add a unit test at the resolver boundary instead.

### Step 9: Update docs

Update `docs/safety.md` with:

- default fetch network deny policy;
- explicit statement that fetch does not crawl, execute JavaScript, or follow arbitrary recursive links;
- special-use address blocking summary;
- relationship between `allow_localhost` and `allow_private_network`;
- warning that enabling private/local fetch is unsafe for untrusted agent prompts unless the host application imposes additional policy.

Update `docs/config.md` with examples for the two flags.

## Testing Requirements

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test fetch_safety
cargo test --all-features --test docs_config_snippets --test docs_tool_names
```

If the full release gate is cheap enough in the environment, run:

```bash
make check
```

## Regression Risks

### Risk: Blocking too broadly

Blocking documentation, multicast, and benchmarking ranges should not affect normal public web fetch. Blocking all of `192.0.0.0/24` may theoretically block special public-service exceptions. For agent SSRF defense, conservative blocking is acceptable. Document the choice.

### Risk: Config compatibility

If users rely on `allow_private_network = true` to fetch localhost, tightening the separation from `allow_localhost` could change behavior. Check existing tests and docs before changing semantics. If changing behavior, call it out in `CHANGELOG.md`.

### Risk: Platform-specific URL parsing

IPv6 literal parsing must use bracketed URLs. Tests should use `url::Url` behavior rather than hand-parsed strings.

## Deliverables

- Updated `src/fetch/limits.rs` with explicit special-use address blocking.
- Tests for direct literal blocked/allowed IPv4 and IPv6 targets.
- Tests for redirect-to-blocked-target behavior.
- Tests for IPv4-mapped IPv6 blocked behavior.
- Updated safety/config docs.
- Changelog entry if semantics change.

## Definition of Done

This milestone is complete when every blocked special-use address class fails by default through the validation path, public examples still validate, fetch tests are deterministic/offline, docs accurately describe the implemented policy, and the release gate remains green.
