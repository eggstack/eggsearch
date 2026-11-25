# M003 — Qualification, Packaging, and Operator Documentation

Status: blocked
Date: 2026-11-24
Subsystem: dynamic-fetch proxy routing
Milestone class: infrastructure (qualification and operator contract)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`
Decision authority: ADR-0025 (accepted); ADR-0002 (eggfetch owns HTTP); ADR-0014 (payload bounds)

References: `plans/implementation/dynamic-fetch-proxy-routing/002-pinned-proxy-routing.md`;
`plans/closure/dynamic-fetch-proxy-routing/001-status.md`; `docs/features.md` § Outbound
Proxy-Chain Routing; `docs/config.md`; `architecture/fetch.md` §8; `make packaging-check`.

## Objective

Qualify the M002 route on an exact candidate and give operators a complete
contract: what is routed, what is refused, what it costs, and how to diagnose
it. No new capability is introduced here.

## Current implementation evidence

- M001 measured the transport: CONNECT authority is the pinned `IP:port`;
  SOCKS5 local resolution is pinned; SOCKS5H and plaintext forward pins are
  refused; `ResolvedTargetRedirect` refuses a pinned cross-origin hop.
- M002 adds the `[fetch.proxy]` surface, the route fingerprint, the typed
  errors, and the `route` response field.
- `docs/features.md` currently documents only `[egress]`, which is
  source-build only and fails closed in prebuilt binaries. The new route is
  always compiled, so the two must be described as distinct features with
  distinct trust boundaries.
- Prebuilt packaging lists features and artifacts in
  `packaging/release-inputs.txt` and the packaging guards; the `proxy`
  dependency feature must appear in the normal build set without adding an
  eggsearch Cargo feature or a second SKU.

## Contract to document

1. **Route scope:** only `fetch`/`batch_fetch` web items. Provider upstreams,
   `repo_fetch`, browser traffic, forge, updater, and rmcp stay direct.
2. **Route policies:** `direct`, `proxy`, `proxy_required`, with the
   `no_proxy` meaning and the `proxy_required` conflict rule.
3. **Supported combinations:** HTTPS through HTTP CONNECT and SOCKS5 with local
   resolution. Refused: SOCKS5H, plaintext HTTP targets, proxy URLs with
   inline userinfo, `https` proxy URLs.
4. **Credentials:** environment-indirected, both-or-neither, resolved per call,
   never logged, never in the cache key, never forwarded to the origin.
5. **Determinism:** proxy environment variables are never read; no PAC, no
   rotation, no per-call override.
6. **Failure behavior:** the named errors, and the guarantee that a failed
   route never silently goes direct.
7. **Cache:** route fingerprint isolation; `cache_only` performs no network and
   still requires a matching fingerprint.
8. **Challenge coupling:** a pass cookie is origin-keyed; changing egress
   address can invalidate a solved challenge. Cookies are never shared across
   routes.
9. **Cost:** measured binary-size delta on the exact candidate.

## Ordered work packages

1. Extend `tests/fetch_proxy_route.rs` with the full failure matrix at the
   eggsearch handler level, including `no_proxy` reporting, `proxy_required`
   configuration refusal, redirect route change, and `cache_only` isolation.
2. Add the packaging assertion that the `proxy` feature is present in default
   and prebuilt builds and that no new artifact appears; run
   `make packaging-check`.
3. Write the operator contract in `docs/features.md` and `docs/config.md`,
   update `architecture/fetch.md` §8, and record the distinction from
   `[egress]` in both places.
4. Update `docs/test-inventory.md`, `architecture/testing.md`, and
   `skills/eggsearch-dev/SKILL.md` for the suite inventory.
5. Measure the release binary and record the delta; confirm the existing
   `egress-feature-qualify` lane is unchanged.
6. Write the closure record on the exact candidate.

## Verification

- `cargo test --locked --all-features --test fetch_proxy_route`
- `cargo test --locked --features mock --test config_validation`
- `make check`
- `make docs-check`
- `make packaging-check`
- Release binary size with the route compiled in, recorded against the M001
  baseline.

## Acceptance and stop conditions

- Pin preservation, fail-closed behavior, and cache isolation are demonstrated
  on the exact candidate, not inferred.
- Direct paths for `repo_fetch`, provider routing, browser, forge, and updater
  are asserted unchanged.
- No new tool, no new Cargo feature, no new artifact, no second SKU.
- Documentation states the refused combinations instead of implying support.
- **Stop:** qualification needs a hosted proxy, a live credential, or a change
  to the M002 contract.

## Non-goals

- New route policies, proxy rotation, PAC, or upstream discovery.
- Proxying any traffic class outside `fetch`/`batch_fetch` web items.
- Multi-hop chains inside this repository; an operator-run loopback relay stays
  the supported chain pattern.
- Re-qualifying the `egress` lane.

## Closure evidence

Failure-matrix results, packaging-check output, size numbers, documentation
diff, unchanged-direct assertions, and the limitations list.
