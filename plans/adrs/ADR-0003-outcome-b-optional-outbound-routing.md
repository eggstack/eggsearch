# ADR-0003: Outcome B Optional Outbound Routing

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#4`
- `plans/001-terminology-and-domain-model.md#4`

Affected subsystem roadmaps:

- `plans/subsystems/optional-outbound-routing-roadmap.md`

## Context

Some operators need provider upstream traffic to traverse an HTTP/SOCKS
proxy chain. Untrusted dynamic fetch targets cannot safely use a route that
loses the authorized resolved-address binding.

## Decision drivers

- Preserve the resolved-address/SSRF contract as the primary acceptance gate.
- Avoid multiple binary SKUs.
- Keep Eggress beneath eggfetch's Dialer boundary, not as a second HTTP stack.

## Considered options

### Option A — Outcome B narrow route (selected)

Eggress owns only listener-free TCP hop establishment via `EggressDialer`.
Provider search engines route through `build_http_client_with_egress`;
dynamic `FetchClient` targets, forge, updater, loopback, rmcp, and browser
paths remain direct. Base protocols `http`/`socks4`/`socks5` only;
credentials env-indirected and redacted. Source-build `egress` opt-in with no
second SKU and no published egress-enabled binary.

### Option B — Route all traffic including dynamic fetch

Rejected: the eggfetch 0.2.0 Dialer seam cannot express pinned
resolved-address routing through the proxy safely.

### Option C — Second egress-enabled SKU

Rejected by the single-SKU policy.

## Decision

Option A (Outcome B). If a path cannot prove the physical route remains bound
to the authorized address snapshot while preserving the logical hostname for
HTTP/TLS, it stays on the pinned direct route.

## Consequences

Positive: operator option without weakening fetch SSRF posture.

Negative: dynamic fetch gains no proxy support; operators must accept direct
egress for untrusted targets.

## Compatibility and migration

Default-feature policy, release SKU policy, and Phase 25/26 security
architecture are unchanged by later maintenance passes.

## Security implications

Malformed-proxy fail-closed behavior, authenticated-path credential
non-forwarding and redaction, routed-redirect ownership, and stalled-
handshake cancellation bounds are required and guarded.

## Verification

Egress route construction guards, authenticated-path and malformed-proxy
regressions, `egress-feature-qualify` lane, default seven-target release
qualification on the exact candidate, and the egress qualify contract check.

## Supersession

None.
