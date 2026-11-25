# Dynamic Fetch Proxy Routing M002 — Pinned Proxy Routing — Status

Status: closed
Date: 2026-11-24
Milestone: M002 — Pinned Proxy Routing for Dynamic Fetch
Decision authority: ADR-0025 (accepted)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set. `[fetch.proxy]` routes the
`fetch`/`batch_fetch` web items through one configured proxy with a mandatory
per-request target pin. No tool, binary feature, or SKU was added.

- `src/fetch/proxy.rs` (new): route policy resolution, endpoint parsing,
  credential lookup from the configured environment variables, the FNV-1a route
  fingerprint, the response route summary, and redacted diagnostics.
- `src/core/config.rs`: `[fetch.proxy]` types, defaults, and validation.
- `src/fetch/client.rs`: `with_proxy_route`, per-hop route selection, and
  `proxy_target_addresses` on proxied hops; the manual redirect loop requires
  the full route decision to stay identical.
- `src/fetch/cache.rs`: `route_fingerprint` on both the raw and derived keys.
- `src/core/fetch.rs`, `src/mcp/tools/web_fetch.rs`: additive `route` response
  metadata and the handler route application; browser-escalated responses keep
  direct cache identity and report no proxy route.
- `src/fetch/types.rs`: the named proxy `FetchError` variants and codes.
- `src/mcp/tools/provider_status.rs`: redacted `fetch_proxy` summary.
- `docs/config.md`: the `[fetch.proxy]` operator contract.
- `tests/fetch_proxy_route.rs` (new), `tests/proxy_route_feasibility.rs`
  (handler-level cases), `tests/config_validation.rs`.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Config rules, defaults, and refusals | `tests/config_validation.rs` (100 passed), including `socks5h`, `https`, inline userinfo, `direct` with fields set, and `proxy_required` with `no_proxy` | pass |
| `no_proxy` bypass is reported, never silent | `proxy_no_proxy_route_is_reported_for_fetch_and_batch_items` | pass |
| Direct route omits metadata | `direct_route_omits_route_metadata` | pass |
| `proxy_required` refuses a plaintext target before any direct request | `proxy_required_http_targets_fail_before_any_direct_request` | pass |
| Unusable endpoint fails closed | `unavailable_proxy_fails_closed_with_a_typed_error` | pass |
| Missing credentials fail before target lookup | `missing_proxy_credentials_fail_before_target_lookup` | pass |
| `cache_only` performs no network under a proxy route | `cache_only_under_proxy_route_does_not_access_the_network` | pass |
| Diagnostics redaction | `provider_status_reports_proxy_summary_without_environment_names` | pass |
| Pinned CONNECT authority and SOCKS5 local resolution at the handler level | `tests/proxy_route_feasibility.rs` handler cases plus the M001 transport matrix | pass |
| Proxy `407` maps to the authentication error | `tests/proxy_route_feasibility.rs` handler case | pass |
| Redirect route change fails closed | `proxy_route_changed_by_redirect` guard in both fetch loops, exercised in `tests/proxy_route_feasibility.rs` | pass |
| Direct and proxied cache entries never collide | `direct_and_proxy_routes_have_distinct_raw_and_derived_cache_keys` | pass |
| `repo_fetch`, provider, site, browser, and workflow paths stay direct | Sole `with_proxy_route` call site is `src/mcp/tools/web_fetch.rs`; no proxy references in browser or repo code | pass |
| Environment proxy variables are never read | No `proxy_environment`/`HTTP_PROXY`/`ALL_PROXY` use in `src/` | pass |

## Verification on the integrated candidate

| Command | Outcome |
|---|---|
| `cargo test --locked --all-features --test fetch_proxy_route` | pass: 7 passed |
| `cargo test --locked --all-features --test proxy_route_feasibility` | pass: 12 passed |
| `cargo test --locked --all-features --lib fetch::proxy` | pass: 6 passed |
| `cargo test --locked --features mock --test config_validation` | pass: 100 passed |
| `cargo test --locked --features mock --test web_fetch_integration` | pass: 130 passed, 1 ignored |
| `cargo test --locked --features mock --test mcp_tools` | pass: 29 passed |
| `RUSTUP_TOOLCHAIN=1.89 make check` | pass |
| `make docs-check` | pass |

## Deviations and design notes

- The route fingerprint is part of **both** the raw and the derived cache key,
  which is stronger than the plan's derived-key-only requirement: a raw hit can
  no longer re-derive content fetched through a different route.
- Browser-escalated responses keep direct cache identity and report no proxy
  route. Browser traffic is never proxied; without this correction a rendered
  response could have claimed a route it did not use.
- `src/fetch/types.rs` was edited to add the named error variants; it sits
  outside the plan's file list and was reported as an ownership exception.
- The redirect guard compares the full `RouteDecision`, not just the route
  class, so a host that starts or stops matching `no_proxy` mid-chain fails
  closed.

## Limitations

- A plaintext `http://` target cannot be fetched through the route: a forward
  proxy cannot carry the pin, so it is refused before I/O.
- `https://` proxy endpoints and `socks5h` are refused at validation.
- The evidence is loopback-only; no hosted proxy is exercised.
