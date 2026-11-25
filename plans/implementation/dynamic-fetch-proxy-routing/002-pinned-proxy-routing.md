# M002 — Pinned Proxy Routing for Dynamic Fetch

Status: closed
Date: 2026-11-24
Subsystem: dynamic-fetch proxy routing
Milestone class: capability
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: ADR-0025 (accepted); ADR-0002 (eggfetch owns HTTP); ADR-0003 (provider route, unchanged)

References: `plans/003-planning-process.md` §2.4; `architecture/fetch.md` §8;
`plans/closure/dynamic-fetch-proxy-routing/001-status.md`; `docs/config.md` § Outbound Proxy-Chain Routing;
vendored `eggfetch-core 0.2.2` `src/proxy.rs` and `src/request.rs`.

## Objective

Give an operator one configured proxy endpoint for URLs an agent supplies to
`fetch`, while the authorized resolved address still binds the physical
connection. When the route cannot honour the pin, the request fails closed with
a typed error instead of silently going direct.

## Current implementation evidence

- `FetchClient::new` (`src/fetch/client.rs:68`) builds a direct transport
  client; `with_timeout_ms` is the existing clone-and-adjust seam.
- Each redirect hop builds the request and applies
  `resolved_addresses(addrs)` (`src/fetch/client.rs:291`, `:1352`).
  `proxy_target_addresses` is the distinct per-request pin for a proxied route
  (`eggfetch-core 0.2.2` `src/request.rs`); it rejects SOCKS5H and plaintext
  HTTP forward proxying.
- `ClientBuilder::proxy` sets the endpoint (`src/client.rs:781`);
  `Proxy::all` parses `http`, `https`, `socks5`, and `socks5h` URLs.
- `FetchSection` (`src/core/config.rs:591`) has no proxy surface;
  `ExtractionCacheKey` (`src/fetch/cache.rs:61`) has no route component.
- `ServerState::fetch_client` (`src/mcp/state.rs:328`) is shared by the
  `fetch`/`batch_fetch` web items and `repo_fetch`; only web items are in
  scope here.

## Contract

```toml
[fetch.proxy]
route = "direct"                  # direct | proxy | proxy_required
url = "http://127.0.0.1:3128"     # http:// (CONNECT) or socks5:// (local resolution)
username_env = "EGGSEARCH_PROXY_USER"
password_env = "EGGSEARCH_PROXY_PASSWORD"
no_proxy = ["localhost", "127.0.0.1", ".internal.example"]
```

Route semantics:

1. `direct` (default): no proxying. `url`, credential variables, and
   `no_proxy` must be absent; otherwise configuration validation fails.
2. `proxy`: every web fetch target uses the proxy except a host matching
   `no_proxy`, which goes direct and reports `route.kind = "direct"` with
   `route.reason = "no_proxy"`.
3. `proxy_required`: every target uses the proxy. A non-empty `no_proxy` is a
   configuration error (`proxy_required_conflicts_with_no_proxy`): there is no
   direct path, so configuration cannot express one.
4. `url` scheme must be `http` or `socks5`. `socks5h` is rejected with
   `proxy_scheme_unsupported`; `https` and inline userinfo are rejected at
   validation. Credentials come only from the two environment variables, must be
   supplied together, and are resolved at call time.
5. Environment proxy variables (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`,
   `NO_PROXY`) are never read. There is no PAC, no rotation, no per-call
   override, and no fallback.
6. Each redirect hop re-runs target validation and re-pins the hop's resolved
   addresses. A hop whose route decision differs from the initial decision
   fails closed with `proxy_route_changed_by_redirect`; a cross-origin redirect
   never downgrades to an unpinned destination.
7. A plaintext `http://` target under `proxy` or `proxy_required` is refused
   with `proxy_target_unsupported` because a forward proxy cannot carry the pin.
8. `cache_only` performs no network and still computes the route fingerprint,
   so a cached entry is only served under the route that stored it.

Response addition: `route: { kind: "direct" | "proxy", reason?, scheme? }`,
present only when `route != "direct"` in configuration. It carries no host,
port, credential, or environment-variable name.

Typed errors, all fail-closed: `proxy_route_unconfigured`,
`proxy_scheme_unsupported`, `proxy_credentials_missing`,
`proxy_authentication_failed`, `proxy_connect_failed`,
`proxy_target_unsupported`, `proxy_route_changed_by_redirect`.

## Bounds

One endpoint per process; one proxy attempt per hop; the existing request
deadline remains authoritative; proxy connect uses the same timeout budget, not
a second outer deadline; no proxy-side retry beyond the existing retry policy.

## Cache identity

Add `route_fingerprint: Option<u64>` to `ExtractionCacheKey`: `None` for
`direct`, otherwise FNV-1a over (route class, proxy scheme, proxy host, proxy
port). Credentials never enter the fingerprint or any identifier. A direct entry
never satisfies a proxied request and vice versa.

## Diagnostics

`provider_status` gains a redacted `fetch_proxy` summary: route class, scheme,
host, port, whether credentials are present, and the validation state. No
credential value, no environment-variable name, and no serialized `Proxy`.

## Ordered work packages

1. Config types, defaults, validation, and `config_validation` coverage.
2. `src/fetch/proxy.rs`: route plan resolution, `Proxy` construction,
   credential lookup, fingerprint, and redacted summary.
3. `FetchClient` route seam: `with_proxy_route`, per-hop client/request
   construction, pin selection, redirect re-evaluation, and typed error mapping.
   Wire into the existing manual redirect loop in `src/fetch/client.rs`, which
   already re-validates and re-resolves each hop; do not delegate redirects to
   eggfetch's internal follower, whose pinned cross-origin hop is refused with
   `ResolvedTargetRedirect`.
4. Handler wiring for the `fetch` and `batch_fetch` web items only; `repo_fetch`
   and workflow fetches stay direct and are asserted to stay direct.
5. Cache key, response `route` field, diagnostics, docs, and schema parity.

## Verification

Focused: extend `tests/proxy_route_feasibility.rs` with the eggsearch-level
matrix (direct, proxy, proxy_required, `no_proxy`, authentication failure,
unreachable endpoint, SOCKS5, SOCKS5H refusal, plaintext-target refusal,
cross-origin redirect, cache isolation); `tests/config_validation.rs` for every
validation rule; `tests/fetch_proxy_route.rs` for the response field,
fail-closed behavior with an unreachable endpoint, and `cache_only` under a
proxy route; `tests/mcp_tools.rs` for schema parity. Broad: `make check`;
`make docs-check`.

## Acceptance and stop conditions

- A proxied HTTPS fetch reaches the origin, the proxy observes the pinned
  `IP:port` as CONNECT authority, and the origin observes the logical
  hostname.
- `socks5h`, plaintext-forward, and unavailable-endpoint cases fail closed with
  their named errors and make no direct request.
- `no_proxy` under `proxy` is reported, never silent.
- Direct and proxied cache entries never collide.
- `repo_fetch`, provider routing, `[egress]`, and browser traffic are unchanged.
- **Stop:** the pin cannot be carried per hop, the route needs a second SKU, or
  the change requires reading proxy environment variables.

## Non-goals

- Proxy servers, relays, or chain building inside this repository.
- Per-call proxy selection, rotation, PAC, or upstream proxy discovery.
- Proxying browser traffic, provider upstreams, forge, or updater traffic.
- A new tool, a new binary feature, or a second SKU.

## Closure evidence

Config validation results, the loopback matrix, CONNECT authority bytes for the
egsearch-level path, fail-closed evidence for every named error, cache isolation
result, unchanged-direct evidence for `repo_fetch`, diagnostics redaction,
documentation diff, and limitations.
