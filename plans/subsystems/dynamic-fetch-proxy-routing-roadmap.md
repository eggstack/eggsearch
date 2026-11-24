# Dynamic-Fetch Proxy Routing Roadmap

Status: proposed (M001 ready; M002 and M003 blocked on M001 and ADR-0025 acceptance)
Planning baseline: `d932115d1f86af51710747d243455792ff63a6fb` (`docs/user-facing-polish-20261123`)
Owner: fetch transport and configuration

References: `plans/000-long-term-specification.md` §4; `plans/001-terminology-and-domain-model.md` §4; ADR-0002 (transport ownership); ADR-0003 (provider upstream routing, superseded for dynamic fetch on acceptance of ADR-0025); ADR-0025 (proposed); `architecture/fetch.md` §8; `docs/features.md` § Outbound Proxy-Chain Routing; `docs/config.md` § Outbound Proxy-Chain Routing.

## 1. Purpose and ownership boundary

Give operators a supported way to route dynamically supplied fetch targets
through a proxy without weakening the resolved-address/SSRF contract. This
workstream owns the `fetch` web-item route selection, the `[fetch.proxy]`
configuration surface, the derived-cache route identity, and the proxy
qualification lane. It does not own HTTP mechanics, pooling, TLS, redirects, or
deadlines (eggfetch), the provider-upstream `[egress]` route (closed roadmap
`plans/subsystems/optional-outbound-routing-roadmap.md`), or browser traffic
(remains direct).

## 2. Work classification

- **Invariant:** the authorized address snapshot binds the physical connection;
  a proxied fetch never reaches an unpinned destination; absence of a usable
  route fails closed; exactly nine advertised tools; provider routing and
  `[egress]` behavior unchanged; no second SKU.
- **Capability:** an operator-configured proxy route for `fetch`/batch web
  items, with per-request target pinning.
- **Infrastructure:** configuration parsing, cache route identity, and a
  loopback proxy harness.
- **Polish:** diagnostics, redaction review, and operator documentation.

## 3. Non-goals

- Eggress as a dynamic-fetch dialer; extending `DialTarget`; changing Eggress.
- A listener, forward-proxy server mode, TLS interception, or MITM support.
- Proxying the browser, forge, updater, loopback health, or rmcp paths.
- SOCKS5H and plaintext HTTP forward proxying for pinned targets.
- Automatic proxy discovery from environment variables in normal operation.
- Per-host routing policy beyond `NO_PROXY` bypass.
- A second binary SKU or a fetch capability that prebuilt artifacts lack.

## 4. Current state

- `src/fetch/egress.rs` implements `EggressDialer` over `eggress-outbound`
  1.0.12 and is gated by the source-build `egress` feature (`Cargo.toml:49`).
- `architecture/fetch.md` §8 states the scope: provider search-engine upstreams
  only; dynamic SSRF-pinned `FetchClient` paths never use the module; forge,
  package-resolver, updater, loopback health, rmcp, and browser traffic stay
  direct.
- `eggfetch-core = "0.2.2"` is pinned without the `proxy` feature
  (`Cargo.toml:70`). The vendored crate exposes `ClientBuilder::proxy`,
  `environment_proxy`, `no_proxy`, per-request `proxy()`, and
  `proxy_target_addresses`; the pin applies to HTTP CONNECT and
  local-resolution SOCKS5 and is rejected for SOCKS5H and plaintext HTTP
  forward.
- No `[fetch.proxy]` configuration exists, and the extraction cache key has no
  route component.

## 5. Target architecture

```text
core config [fetch.proxy] -> route policy (direct | proxy | proxy_required)
                                    |
fetch web item -> route resolution -> eggfetch client (proxy feature)
                                    |
                          pinned target addresses (mandatory)
                                    |
                          HTTP CONNECT / SOCKS5 local resolution
                                    |
                       Eggress remains provider-upstream only
```

Multi-hop is supported by pointing the single endpoint at an operator-run
loopback relay. The relay is not part of the trust boundary; pinning still
happens in eggfetch.

## 6. Dependency graph

```text
ADR-0025 acceptance -- hard --> M002 implementation
M001 feasibility evidence -- hard --> ADR-0025 acceptance
M002 implementation -- hard --> M003 qualification
```

## 7. Milestones

| Milestone | Class | Status | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|---|
| M001 — Route feasibility and Eggress compatibility | infrastructure | ready | `plans/implementation/dynamic-fetch-proxy-routing/001-route-feasibility-and-eggress-compatibility.md` | None | A recorded route matrix, a loopback proof of pinned CONNECT, and an accept-or-reject recommendation for ADR-0025 |
| M002 — Pinned proxy routing for dynamic fetch | capability | blocked | Not drafted until ADR-0025 is accepted | Hard: M001 evidence and ADR-0025 acceptance | An operator enables `[fetch.proxy]` and a `fetch` request reaches the origin through the proxy, pinned, or fails closed with a typed error |
| M003 — Qualification, packaging, and operator documentation | infrastructure | blocked | Not drafted | Hard: M002 | Proxy lane, failure matrix, size measurement, and docs updated on an exact candidate |

## 8. Cross-cutting requirements

- **Security:** pin before dial; refuse rather than downgrade; redact
  credentials; never forward proxy authorization to the origin; keep loopback
  and private-address refusals.
- **Compatibility:** additive config; no change to `[egress]`, tool names,
  schemas beyond additive optional fields, or provider behavior.
- **Cache:** route identity (scheme, endpoint, pin class) joins the derived key;
  a cached entry never satisfies a request under a different route.
- **Failure and timeouts:** proxy connect failures map to typed errors; the
  existing deadline owner stays authoritative; no second outer deadline.
- **Observability:** route state appears in `provider_status` diagnostics as a
  redacted summary, never with credentials or env names.
- **Packaging:** the `proxy` feature enters the normal build set; no new SKU;
  binary-size delta is measured and recorded.

## 9. Verification strategy

Loopback-only harnesses (no hosted proxies): HTTP proxy with CONNECT, SOCKS5
with local resolution, authentication success and failure, malformed route,
timeout, cross-origin redirect, `NO_PROXY` bypass, SOCKS5H refusal, plaintext
forward refusal, and cache isolation. Focused suites plus `make check`,
`make docs-check`, and the existing `egress-feature-qualify` lane unchanged.

## 10. Risks and decision points

- **CONNECT authority semantics.** The pin is only real if CONNECT names the
  pinned address while Host and SNI stay logical. M001 must prove this against a
  loopback proxy rather than trusting documentation.
- **Challenge and cookie binding.** Pass cookies are origin-keyed; a different
  egress address can invalidate a solved challenge. The failure is acceptable
  but must be documented, and cookies must never be shared across routes.
- **Cache correctness.** A route-blind key would let one route's content serve
  another; the key change is mandatory, not polish.
- **Feature size.** Enabling `proxy` grows the dependency set; measure before
  and after.
- **Scope creep toward a proxy server.** Eggress's relay use stays outside this
  repository's process boundary.

## 11. Completion definition

M001 evidence is recorded and ADR-0025 is accepted or rejected on that
evidence. If accepted, M002 and M003 land with a qualification record showing
pin preservation, fail-closed behavior, cache isolation, and unchanged provider
routing on the exact candidate.
