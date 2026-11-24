# M001 — Route Feasibility and Eggress Compatibility

Status: ready
Date: 2026-11-24
Subsystem: dynamic-fetch proxy routing
Milestone class: infrastructure (feasibility and decision evidence)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: ADR-0025 (proposed); ADR-0002 (eggfetch owns HTTP); ADR-0003 (provider route)

References: `plans/003-planning-process.md` §2.4 and §6; `architecture/fetch.md` §8; `docs/features.md` § Outbound Proxy-Chain Routing; vendored `eggfetch-core 0.2.2` `src/transport/dialer.rs` and `src/request.rs`.

## Objective

Decide, on measured evidence, whether dynamic fetch can be proxied while the
authorized address snapshot still binds the connection. Produce the route
matrix and the loopback proof that ADR-0025 needs to move from `proposed` to
`accepted` or `rejected`.

## Evidence already gathered (2026-11-24, this worktree)

- `EggressDialer` receives `DialTarget { host, port }`; the type documents that
  it "contains only the URL's logical host and effective port". No address
  vector crosses the dialer seam, so a proxy reached through Eggress resolves
  the origin name itself.
- `eggfetch-core 0.2.2` `proxy_target_addresses` pins HTTP CONNECT and
  local-resolution SOCKS5 to caller-supplied addresses "without an origin DNS
  lookup", and rejects SOCKS5H and plaintext HTTP forward proxying.
- `Proxy` carries a separate direct-only `resolved_addresses` field; the two
  pins are not interchangeable.
- `Cargo.toml:70` pins `eggfetch-core 0.2.2` without the `proxy` feature;
  `Cargo.toml:49` keeps `egress` source-build only.
- `docs/features.md` records that prebuilt and default binaries exclude
  `egress` and fail closed when `[egress]` is enabled.

These are documentation-level facts. M001 exists because the decisive claims
(CONNECT names the pinned address; no origin DNS occurs) must be observed.

## Work packages

1. **Capability probe.** In a scratch branch, enable eggfetch's `proxy` feature
   and construct a client with an explicit proxy plus
   `proxy_target_addresses`. No production source change is retained.
2. **Loopback harness.** Stand up a test-only HTTP proxy and a SOCKS5 proxy
   bound to `127.0.0.1` in the test process. Record the CONNECT authority byte
   for byte, whether the proxy received a hostname or an address, and whether
   any origin DNS query occurred.
3. **Matrix run.** For each combination, record routed outcome, pin outcome, and
   error code: HTTP CONNECT over HTTPS; HTTP forward over plaintext; SOCKS5
   local resolution; SOCKS5H; proxy authentication success and failure; proxy
   TLS endpoint; malformed route; connect timeout; cross-origin redirect;
   `NO_PROXY` bypass; loopback destination refusal.
4. **Eggress comparison.** Run the same matrix through `EggressDialer` with a
   single hop, and record exactly where pinning is lost and whether the loss is
   observable to the caller.
5. **Recommendation.** Write the route matrix, the size delta, and an
   accept-or-reject recommendation into ADR-0025 and this plan's closure record.

## Bounds and safety

The harness is loopback-only, uses self-signed certificates generated in the
test, and makes no hosted request. No credential is written to a fixture. The
probe branch is discarded; only evidence and plans are retained.

## Verification

- `cargo test --locked --all-features --test` a new `proxy_route_feasibility`
  suite containing the harness (added when M002 starts; M001 may run the probe
  as an untracked scratch test).
- Recorded evidence: CONNECT authority, DNS observation, matrix table, size
  delta for the `proxy` feature.
- `make check` and `make docs-check` pass with only documentation changes.

## Acceptance and stop conditions

- **Accept:** every pinned combination reaches the intended destination with the
  proxy never resolving the origin name, and every unpinnable combination fails
  closed with a distinguishable error. ADR-0025 can be accepted.
- **Reject:** if CONNECT cannot carry the pinned address while Host and SNI stay
  logical, or if the pin is silently dropped on any path a user can reach,
  record the gap, reject ADR-0025, and leave dynamic fetch direct.
- **Stop:** the probe requires a hosted proxy, a credential, or a production
  behavior change to answer the question.

## Non-goals

- Implementing `[fetch.proxy]`, touching `src/fetch/egress.rs`, or changing
  default features.
- Browser, forge, updater, rmcp, or loopback proxying.
- Multi-hop chains inside this repository.

## Closure evidence

Route matrix with per-combination outcomes, the CONNECT capture, the DNS
observation method, size delta, ADR-0025 status change (or rejection note),
`make check` and `make docs-check` results, and an explicit limitations list.
