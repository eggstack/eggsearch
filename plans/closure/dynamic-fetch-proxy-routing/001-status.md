# Dynamic Fetch Proxy Routing M001 — Status

Status: closed
Date: 2026-11-24
Milestone: M001 — Route Feasibility and Eggress Compatibility
Decision authority: ADR-0025 (accepted on this evidence)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the worktree at baseline `d932115d1f86af51710747d243455792ff63a6fb`
plus this change set. The production change is one manifest line: the pinned
`eggfetch-core 0.2.2` dependency now enables `proxy` (`Cargo.toml`), with the
corresponding dependency-edge update in `Cargo.lock`. The evidence comes from
`tests/proxy_route_feasibility.rs`, a loopback-only suite with in-process HTTP
CONNECT, HTTP forward, SOCKS5, TLS-origin, and HTTPS-proxy fixtures. No hosted
request, credential, or production behavior change is involved.

## Route matrix (measured)

| Route case | Observed outcome | Verdict |
|---|---|---|
| HTTPS over HTTP CONNECT with `proxy_target_addresses` | Request succeeded for `https://route-probe.invalid:<port>/`. Proxy captured `CONNECT 127.0.0.1:<pinned-port> HTTP/1.1`; origin saw `Host: route-probe.invalid`; the self-signed certificate for the logical name validated. | pinned |
| SOCKS5 with local resolution and the pin | SOCKS server captured `SocksTarget { atyp: 1, host: "127.0.0.1", port: <pinned> }`; logical Host retained. | pinned |
| SOCKS5H with the pin | Rejected before proxy I/O: `Unsupported("proxied destinations are incompatible with SOCKS5H remote DNS")`. Unpinned SOCKS5H sent `atyp: 3` with the hostname and failed at the proxy (`ProxyConnect("SOCKS5 CONNECT failed: host unreachable")`). | fails closed |
| Plaintext HTTP forward proxy | Unpinned forward succeeded. Adding the pin was rejected before proxy I/O: `Unsupported("proxied destinations are unsupported for plaintext HTTP forward proxies")`. | fails closed |
| Proxy authentication | Correct Basic credentials accepted; wrong credentials returned `ProxyConnectRejected { status: 407, body: "" }`. | typed error |
| HTTPS proxy endpoint | Succeeded with distinct proxy and origin trust roots; the two trust stores do not contaminate each other. | supported |
| Malformed proxy route | `InvalidProxyUrl("invalid proxy URL: relative URL without a base")`. | typed error |
| Stalled CONNECT | `Timeout { phase: Connect, elapsed: 150ms }`. | typed error |
| Cross-origin redirect with a pinned target | `ResolvedTargetRedirect`; the transport refuses rather than re-pinning a new origin. | fails closed |
| `NO_PROXY` bypass | Direct fixture succeeded with zero proxy accepts. The in-repo fetch validator independently returned `PrivateNetworkBlocked("address not allowed by policy: 127.0.0.1")`. | explicit bypass |
| Eggress dialer (single hop) | The proxy captured `CONNECT route-probe.invalid:<port> HTTP/1.1`: the logical hostname reaches the proxy, so the proxy would resolve origin DNS. A caller-supplied resolved-address pin alongside a custom dialer is refused before dial: `Unsupported("custom dialing is incompatible with caller-supplied resolved destinations")`. | pin lost |

## DNS observation

The origin hostname is a reserved `.invalid` name that has no usable DNS answer.
The pinned-address route succeeded anyway, and the proxy received only the
pinned loopback `IP:port` as CONNECT authority. That is consistent with no
origin DNS lookup by the client, and it is inconsistent with a name-resolving
route. This is an indirect observation, not packet-level DNS telemetry: the
harness does not prove that no DNS query was attempted, only that success does
not depend on one.

## Transport details recorded

- HTTP CONNECT authority is the pinned address; `Host` and TLS identity stay
  logical. The observed request head omitted the non-default origin port from
  `Host` (`Host: route-probe.invalid`). This is an eggfetch transport detail,
  not an eggsearch decision, and M002 must not assert the port form.
- The proxy pin survives only on the paths eggfetch accepts. Anything else is a
  typed rejection before proxy I/O, never a silent downgrade.

## Verification

| Command | Result |
|---|---|
| `cargo test --locked --all-features --test proxy_route_feasibility` | pass: 9 passed, 0 failed |
| `rustfmt --check --edition 2021 tests/proxy_route_feasibility.rs` | pass |
| `cargo build --release` with `proxy` disabled | 27,214,144 bytes |
| `cargo build --release --locked` with `proxy` enabled | 28,194,960 bytes |
| Size delta attributable to the feature | +980,816 bytes (+3.60%) |
| `RUSTUP_TOOLCHAIN=1.89 make check` | pass on the integrated candidate: fmt, clippy, no-default check, all-features tests, hygiene, dependency policy, packaging |
| `make docs-check` | pass |
| Final release binary (stable, all milestones) | 28,345,248 bytes |

## Recommendation

**Accept ADR-0025.** Every pinned combination reaches the intended destination
without the proxy resolving the origin name, and every unpinnable combination
fails closed with a distinguishable error. Eggress cannot carry the pin: it
sends the logical hostname, so its proxy resolves origin DNS.

## Limitations

- Indirect DNS observation, as described above.
- The size delta is measured on one local macOS arm64 release build.
- The suite proves transport behavior; it does not qualify the eggsearch
  `[fetch.proxy]` surface, which is M002.
- The `egress` comparison runs only under `--features egress` and exercises a
  single hop.
