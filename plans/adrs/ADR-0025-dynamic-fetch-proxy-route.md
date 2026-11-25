# ADR-0025: Dynamic-Fetch Proxy Routing Ownership

Status: accepted

Date: 2026-11-24

Acceptance evidence: `plans/closure/dynamic-fetch-proxy-routing/001-status.md`
(loopback route matrix and pinned-CONNECT capture, 2026-11-24).

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4
- `plans/001-terminology-and-domain-model.md` §4

Affected subsystem roadmaps:

- `plans/subsystems/dynamic-fetch-proxy-routing-roadmap.md`
- `plans/subsystems/optional-outbound-routing-roadmap.md` (closed; retained as history)

Supersedes: ADR-0003, for the dynamic-fetch exclusion only. The
provider-upstream route and Eggress ownership decision from ADR-0003 are
restated here unchanged, so no part of ADR-0003 is silently dropped.

## Context

ADR-0003 chose Outcome B: Eggress routes fixed operator-owned provider
upstreams, and dynamic `FetchClient` targets stay direct. Its stated reason was
that "the eggfetch 0.2.0 Dialer seam cannot express pinned resolved-address
routing through the proxy safely".

Two things changed after that decision. eggfetch-core 0.2.2, which eggsearch
already pins (`Cargo.toml:70`), now ships a `proxy` feature with a request-level
`proxy_target_addresses` pin. Its own contract reads: "HTTP CONNECT and
local-resolution SOCKS5 use these addresses without an origin DNS lookup.
SOCKS5H and plaintext HTTP forward-proxy requests reject this option." Separately,
operators behind mandatory egress control have no supported way to fetch an
agent-selected URL.

Eggress itself is unchanged: `EggressDialer` implements eggfetch's `Dialer`,
and `DialTarget` still carries only a logical host and port
(`transport/dialer.rs`), never an address snapshot.

## Decision drivers

- The SSRF pin must survive routing: the connection must reach the authorized
  resolved address, not whatever the proxy resolves.
- No second binary SKU and no feature that prebuilt artifacts silently lack.
- Eggfetch stays the HTTP, pooling, TLS/SNI, decompression, redirect, and
  deadline owner (ADR-0002).
- Proxy credentials are environment-indirected, redacted, and never forwarded
  to the origin.
- Absence of a usable route fails closed; a proxy is never silently bypassed.

## Considered options

### Option A — Eggress for dynamic fetch (rejected)

`DialTarget` cannot carry the authorized address vector, so the proxy performs
origin DNS resolution and the local validate-and-pin step stops binding the
physical destination. The `egress` feature is also source-build-only: it is
excluded from default features, prebuilt binaries, and the Python wheels, and an
enabled route in a binary without the feature fails closed (`docs/features.md`).
A fetch-facing capability gated behind it would be missing from every artifact
users install.

### Option B — eggfetch-native pinned proxy (selected)

Enable eggfetch's `proxy` feature and require `proxy_target_addresses` on every
proxied fetch of a dynamically supplied URL. The pin covers HTTPS through HTTP
CONNECT and SOCKS5 with local resolution. The two combinations eggfetch rejects
(plaintext HTTP through an HTTP forward proxy, and SOCKS5H) are refused with a
typed error rather than downgraded to an unpinned route.

### Option C — Loopback relay plus pinned endpoint (permitted for chains)

An operator who needs a multi-hop chain runs a relay bound to loopback (any
proxy implementation, including an Eggress-based one) and configures eggfetch's
single proxy endpoint at that relay. eggfetch still pins the ultimate target, so
the pin is preserved end to end. No Eggress code enters the fetch path, and the
relay stays outside the trust boundary.

### Option D — Status quo (insufficient)

Direct-only dynamic fetch remains the fallback for operators who cannot egress
directly, which in practice means no fetch at all.

## Decision

Dynamic fetch gains proxy routing only through eggfetch's native `proxy`
feature with a mandatory request-level target pin. Eggress remains the
provider-upstream chain builder it is today and is not extended into the
dynamic-fetch path. Multi-hop needs are met by Option C.

## Consequences

Positive: operators get a supported route; the SSRF contract is enforced at the
only place that can enforce it; no second SKU.

Negative: `proxy` becomes part of the dependency feature set, which increases
binary size; plaintext HTTP and SOCKS5H remain unsupported for pinned fetch;
proxy behavior becomes a new cache-identity input.

## Compatibility and migration

Additive `[fetch.proxy]` configuration, disabled by default. Native proxy
support never reads proxy environment variables unless the operator opts in to
an explicit environment snapshot; that keeps stdio MCP behavior deterministic.
Existing `[egress]` configuration and provider routing are unchanged. Derived
cache entries created before the change remain valid because the route
fingerprint is additive to the key.

## Security implications

Pinned targets must be validated before use; `NO_PROXY` and loopback/private
target refusals keep their current meaning; a redirect to a new origin re-runs
validation and pinning; `Proxy-Authorization` never reaches the origin; proxy
endpoint TLS is configured separately from origin TLS; an enabled route with no
usable pin fails closed with a typed error.

## Verification

A loopback HTTP and SOCKS5 proxy harness proves the CONNECT authority is the
pinned address and that no origin DNS lookup occurs; a failure matrix covers
malformed routes, authentication failure, timeouts, redirect cross-origin, and
the rejected SOCKS5H and plaintext-forward combinations; `make check` and
`make docs-check` pass; the existing `egress-feature-qualify` lane is
unchanged.

## Supersession

This ADR supersedes ADR-0003 for dynamic fetch only. ADR-0003 remains accepted
for the provider-upstream route, and its status line records the partial
supersession. ADR-0003 is not rewritten.
