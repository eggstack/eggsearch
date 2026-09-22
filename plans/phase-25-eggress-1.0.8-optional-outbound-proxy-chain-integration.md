# Phase 25 — Eggress 1.0.8 optional outbound proxy-chain integration

Status: planned / ready for handoff

Planning baseline: dfa90e050c5434f3346902aeb4074901c58e90d1 (eggsearch 0.3.9 on main)

Depends on:

- Phase 24 eggfetch 0.2.0 adoption implemented;
- published eggfetch-core 0.2.0 with the custom Dialer seam;
- published eggress-outbound 1.0.8 and eggress-uri 1.0.8;
- Eggress workspace MSRV 1.89, matching eggsearch;
- Eggress 1.0.8 release commit f0affac49c0fdaf6bcb51dfe1cb47f1f6548ffed;
- Eggress HTTP CONNECT consolidation commit b5193688b2478bb0e9c24279d29e5a3097c03d67.

## Context

Eggsearch already has a clean HTTP ownership boundary after Phases 17 and 24:
eggfetch-core owns HTTP framing, connection pooling, destination TLS/SNI,
compression, redirect mechanics, and transport errors, while eggsearch retains
provider policy, SSRF authorization, bounded reads, application retries,
sanitization, parsing, and release qualification.

Eggress 1.0.8 now provides a listener-free Rust outbound API in
eggress-outbound. OutboundConnector can open TCP streams through ordinary
HTTP/SOCKS proxy chains without starting an Eggress listener or embedding the
full Eggress service. Its detailed TCP failure surface exposes bounded,
credential-safe kind/stage/hop/protocol facts.

Eggfetch 0.2.0 exposes an application-owned Dialer seam that accepts a logical
host/port and returns a Tokio-compatible byte stream while eggfetch remains
responsible for HTTP and destination TLS. This is the intended composition
point.

The integration must not replace eggfetch, start a localhost proxy, or add
eggress-embed. Eggress is only a physical outbound-route provider beneath
eggfetch.

A security constraint is first-class in this phase. Eggsearch web_fetch
currently validates DNS and passes the approved resolved-address snapshot back
to eggfetch so the connection cannot drift to a different DNS answer. The
current custom-Dialer route must be proven to preserve an equivalent
resolved-address/SSRF contract before it is used for untrusted dynamic fetch
targets. If that cannot be done cleanly, web_fetch and any equivalent
SSRF-sensitive path remain on their existing pinned route.

## Objective

Add a narrowly scoped Eggress integration that gives eggsearch an optional,
in-process, listener-free HTTP/SOCKS proxy-chain route without changing the
default direct-network behavior or weakening any current safety contract.

Desired end state:

- eggfetch remains the sole HTTP client and destination-TLS owner;
- Eggress owns only proxy-chain TCP establishment;
- no Eggress listener, service lifecycle, admin endpoint, system-proxy mutation,
  or full embed runtime is introduced;
- direct/default eggsearch behavior is byte-for-byte policy equivalent when
  Eggress routing is not configured;
- the first supported Eggress profile is ordinary TCP HTTP/SOCKS routing only;
- advanced Eggress transports and compatibility surfaces remain excluded unless
  separately planned;
- route failures fail closed and never silently fall back to direct;
- proxy credentials are not serialized in eggsearch configuration, logs, or
  diagnostics;
- retry policy remains owned by eggsearch/OriginController;
- request/total timeout ownership remains with eggfetch/eggsearch rather than
  adding a second outer Eggress deadline;
- untrusted dynamic-target SSRF protections remain at least as strong as the
  current resolved-address pinning behavior;
- dependency, binary-size, startup, throughput, and cross-platform impact are
  measured before deciding whether Eggress belongs in the default/release build.

## Non-negotiable ownership boundary

    eggsearch
      owns route selection policy, configuration, credential indirection,
      SSRF authorization, redirect authorization, retries/circuit breaking,
      bounded reads, provider/fetch semantics, diagnostics, and qualification

    eggfetch-core
      owns HTTP/1 framing, pooling, destination TLS/SNI/certificate checks,
      decompression, redirect mechanics, request/total deadlines, and the
      Dialer abstraction

    eggress-outbound
      owns physical TCP route construction and proxy-hop handshakes only

    rmcp
      retains its existing independent Streamable HTTP reqwest transport

Do not duplicate HTTP CONNECT, SOCKS, TLS, or proxy-chain protocol machinery in
eggsearch.

## Initial dependency and feature budget

Prefer the smallest Eggress surface:

- eggress-outbound = 1.0.8, default-features = false;
- eggress-uri = 1.0.8 only if direct construction/parsing requires it;
- no eggress-embed;
- no eggress-runtime/server/admin/system-proxy;
- no pproxy-compat initially;
- no extended, pproxy-legacy, legacy-crypto, ssh, quic, insecure-tls, or udp
  features;
- no Git/path overrides.

The base phase targets ordinary TCP HTTP/SOCKS proxy chains. Do not use this
integration as justification to expose every Eggress protocol.

Introduce a single eggsearch Cargo feature for the integration, provisionally
named egress. Do not create multiple binary SKUs.

## Work item 1 — Freeze baseline and characterize the current transport graph

Before production edits:

1. record the exact current main SHA;
2. record cargo tree -e normal;
3. record cargo tree -e features -i eggfetch-core;
4. confirm direct eggsearch reqwest remains absent;
5. locate every eggfetch Client construction site;
6. classify each client/path as:
   - fixed/operator-owned upstream;
   - user-controlled or redirect-derived dynamic target;
   - SSRF-pinned;
   - updater/release;
   - rmcp-owned and out of scope;
7. record representative default release binary size and startup RSS if the
   existing measurement harness makes this cheap.

Do not assume one shared client factory currently covers every network path.

## Work item 2 — Prove the Eggfetch Dialer compatibility boundary before wiring Eggress

Build a focused compile/test prototype around eggfetch-core 0.2.0 Dialer.

Prove:

- a custom Dialer can return an Eggress BoxStream without adapters that copy
  payload bytes;
- eggfetch still performs destination TLS and SNI above that stream;
- connection pooling occurs above the Dialer so a proxy handshake is not
  repeated for every request;
- dropping/cancelling an in-flight eggfetch connection future safely cancels
  Eggress chain establishment;
- eggfetch request/total deadlines remain authoritative;
- no second retry loop or direct fallback appears.

Use OutboundConnector::connect_tcp_detailed, not the Eggress outer-timeout
wrapper, unless testing proves an Eggress-owned timeout is required for an
internal hop and does not conflict with the caller deadline.

Map Eggress detailed failure categories into eggfetch DialErrorKind without
string parsing:

- Timeout -> Timeout;
- Authentication -> Authentication;
- Policy -> Rejected;
- DNS / refused / network-unreachable / host-unreachable -> Connection;
- TLS / protocol / other -> Other.

Retain the bounded Eggress error as the nested source where safe so diagnostics
remain useful without leaking credentials.

## Work item 3 — Resolve the SSRF/resolved-address pinning gate

This is a release blocker, not an optional test.

web_fetch currently performs target validation, obtains an approved resolved
address set, and supplies that set to eggfetch for the physical connect. A proxy
that re-resolves the original hostname can otherwise defeat the local DNS
snapshot and reintroduce DNS-rebinding/private-network exposure.

Determine exactly how eggfetch-core 0.2.0 handles the combination of:

- custom Dialer;
- per-request resolved_addresses;
- destination hostname/SNI;
- physical connect target.

Preferred Outcome A:

- preserve the logical hostname for HTTP Host and TLS SNI;
- pass the already-authorized physical IP/port information into the custom
  route so Eggress reaches exactly an approved destination;
- keep redirect-hop revalidation and repinning unchanged.

If eggfetch 0.2.0 cannot express this safely, do not emulate it with a
process-global mutable hostname-to-IP cache.

Outcome B:

- keep web_fetch and every equivalent untrusted dynamic-target path on the
  existing direct pinned route;
- allow Eggress only for fixed/operator-owned upstream paths whose target is not
  supplied by an untrusted MCP caller;
- document this scope explicitly;
- open or register a separate eggfetch enhancement only if a small general
  resolved-target-aware Dialer extension is warranted.

No implementation may route an untrusted hostname through remote proxy DNS
after merely validating a different local DNS answer.

## Work item 4 — Add the EggressDialer adapter behind a narrow module boundary

Add one small transport-routing module rather than spreading Eggress types
through providers and fetch code.

Responsibilities:

- own Arc<OutboundConnector>;
- implement eggfetch_core::Dialer;
- translate DialTarget host/port to OutboundConnector;
- translate typed Eggress failures;
- expose no provider/search/fetch policy;
- contain feature-gated Eggress imports;
- avoid logging full proxy URIs or credentials.

Eggress types must not leak into the stable MCP/CLI response schemas.

Add focused unit tests for:

- direct connector smoke;
- HTTP proxy chain;
- SOCKS5 proxy chain;
- at least one two-hop mixed HTTP/SOCKS chain;
- authentication rejection;
- unavailable first hop;
- malformed/unsupported chain configuration;
- timeout/cancellation;
- credential-safe Display/Debug;
- no direct fallback after chain failure.

All normal tests remain local and deterministic.

## Work item 5 — Add operator configuration without storing raw secrets

Add a top-level route configuration because egress affects multiple eggsearch
HTTP consumers rather than only web_fetch.

Keep the schema minimal and typed. Prefer a native list of hops that can be
translated into Eggress native chain types. The configuration must support at
least:

- route disabled/direct by default;
- ordered HTTP/SOCKS hop definitions;
- host and port;
- credential environment-variable references rather than raw passwords;
- explicit validation errors for unsupported schemes/features.

Do not make credential-bearing proxy URIs the canonical persisted config
format. If a URI convenience input is retained, it must be optional, redacted,
and never written back by AppConfig::save with embedded secrets.

When eggsearch is compiled without the egress feature:

- default/no-egress configuration continues to load normally;
- an explicitly configured Eggress route fails clearly instead of being
  silently ignored.

Update architecture/config.md and example configuration only after the runtime
contract is stable.

## Work item 6 — Centralize route-aware eggfetch client construction

Do not add per-provider Eggress setup.

Create or extend one client-construction seam that can build:

- the existing direct/pinned eggfetch client; or
- an eggfetch client using the EggressDialer for eligible paths.

Preserve:

- current user-agent behavior;
- current follow_redirects(false) ownership;
- current compression feature behavior;
- Phase 23 timeout override semantics;
- shared-client reuse for equal/shorter timeout overrides;
- one widened client per top-level longer-timeout operation;
- provider-specific request construction;
- OriginController retry/circuit ownership.

A longer timeout override must rebuild the same selected route, not accidentally
fall back to direct.

## Work item 7 — Apply the route only to qualified consumers

Use the classification from Work item 1.

For Outcome A from the SSRF gate, apply the selected route consistently to
eggsearch-owned HTTP consumers whose existing safety semantics can be preserved.

For Outcome B, constrain Eggress to the explicitly qualified
fixed/operator-owned consumers and leave dynamic SSRF-sensitive paths direct.

In both outcomes:

- rmcp transport remains untouched;
- browser subprocess traffic is not automatically proxied unless separately
  supported and documented;
- no environment-variable proxy auto-discovery is added implicitly;
- failure of the configured chain is a request failure, never a trigger for
  direct fallback.

Static guards should prevent accidental Eggress use in excluded paths when
Outcome B is selected.

## Work item 8 — Prove end-to-end HTTP semantics above Eggress

Use deterministic loopback proxy fixtures.

Required coverage:

- plain HTTP through HTTP proxy;
- HTTPS destination through HTTP CONNECT while eggfetch owns destination TLS;
- HTTP/HTTPS through SOCKS5;
- mixed two-hop chain;
- connection reuse across repeated requests;
- redirect handling remains eggsearch-owned;
- gzip and Brotli responses still decode correctly;
- bounded response bodies remain bounded on decoded bytes;
- malformed proxy replies fail safely;
- proxy authentication does not reach the destination as a normal header;
- chain failure does not bypass to direct;
- cancellation and deadline behavior stay bounded.

Do not duplicate Eggress protocol conformance tests. These are composition tests
for the eggsearch -> eggfetch -> Eggress seam.

## Work item 9 — Re-prove SSRF and redirect safety

For every route-enabled dynamic target permitted by Work item 3, add regression
coverage proving:

- private/loopback targets remain blocked according to FetchLimits;
- redirect targets are revalidated before the next hop;
- DNS rebinding cannot cause the physical route to differ from the approved
  snapshot;
- destination TLS verifies the logical hostname rather than a proxy address;
- proxy-chain DNS behavior cannot bypass local policy;
- credentials embedded in fetched URLs remain rejected.

If any of these cannot be demonstrated deterministically, that target class
must remain outside the Eggress route.

## Work item 10 — Dependency and footprint qualification

Measure both the existing build and the Eggress-enabled build.

Record:

- cargo tree delta;
- duplicate versions introduced;
- enabled Eggress crates/features;
- release binary size;
- clean-build time when practical;
- startup RSS when practical;
- representative request latency;
- pooled repeated-request latency;
- proxy-chain establishment cost.

The default build must not accidentally pull Eggress when the feature is
disabled.

Before closure make one explicit packaging decision:

Outcome A — default/release inclusion:
use only if the dependency and binary delta are acceptable and the complete
safety/behavior suite is green. Keep one canonical binary SKU.

Outcome B — source-build opt-in:
keep the egress feature non-default and document that prebuilt/default builds
do not provide it. Do not create a second binary SKU merely for this phase.

If release binaries are built with a feature set different from Cargo defaults,
that fixed feature contract must be machine-checked in packaging scripts and
documented; do not allow silent drift.

## Work item 11 — Cross-platform and exact-MSRV verification

At minimum run:

    cargo fmt --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo check --locked --no-default-features
    cargo check --locked --features egress
    cargo test --locked --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
    make hygiene
    make packaging-check
    make release-check
    cargo publish --dry-run --locked
    cargo +1.89.0 check --locked --all-features

Confirm:

- no OpenSSL/native-tls dependency is introduced;
- Eggress remains on its rustls path;
- no SSH/russh/RSA dependency appears unless separately authorized;
- no QUIC/H3/UDP/legacy-crypto dependencies appear;
- all current target triples still compile.

## Work item 12 — Seven-target release qualification

If the final production candidate changes the default or release binary graph,
run the existing non-publishing release qualification against the exact
immutable candidate:

    mode = qualify
    ref = <candidate SHA>

Require all seven existing targets and exact final asset-set assembly.

If Outcome B leaves the canonical release binary graph unchanged, still run
cross-target compile qualification for the egress feature where practical and
record why a full release-asset requalification is or is not required.

Do not publish a crate or GitHub Release as part of this phase without separate
authorization.

## Work item 13 — Documentation and operator diagnostics

Update maintained current-state docs to describe:

- whether Eggress support is compiled into the normal binary;
- supported base protocols;
- configuration examples without raw credentials;
- failure-closed behavior;
- which traffic classes use the configured route;
- explicit exclusions such as rmcp/browser or dynamic web_fetch under Outcome B;
- timeout/retry ownership;
- troubleshooting for DNS, authentication, hop handshake, and policy failures.

Diagnostics may expose kind/stage/hop/protocol but must not expose credentials
or complete secret-bearing route strings.

Update architecture/maintenance.md so the transport ownership table records the
new optional physical-route layer.

## Work item 14 — Close with an implementation record

Append exact evidence to this plan:

- starting SHA;
- implementation SHA;
- selected SSRF outcome and proof;
- selected packaging outcome;
- resolved Eggress versions/features;
- final configuration schema;
- eligible/excluded network consumers;
- deterministic route/composition test results;
- dependency and binary deltas;
- exact MSRV result;
- local gates;
- cross-target/release qualification evidence;
- documentation changes;
- deviations/blockers.

Then mark Phase 25 implemented and update plans/registry.md in the same closure
change.

If the SSRF-safe custom-Dialer composition is blocked, mark the phase blocked
rather than weakening web_fetch policy. Any required eggfetch enhancement must
remain general-purpose and independently useful; do not specialize eggfetch for
eggsearch.

## Non-goals

Phase 25 does not:

- replace eggfetch with Eggress;
- add eggress-embed or start an in-process/local proxy listener;
- change the ten-tool MCP surface;
- add proxy-control MCP tools;
- alter ranking/provider selection;
- move retry/circuit policy into Eggress;
- add automatic direct fallback;
- add pproxy compatibility unless separately justified;
- enable Shadowsocks, Trojan, WebSocket, SSH, QUIC/H3, UDP, legacy crypto, or
  insecure TLS;
- proxy browser subprocess traffic implicitly;
- replace rmcp's Streamable HTTP client;
- weaken SSRF/redirect validation to make proxying easier;
- create multiple binary SKUs;
- publish eggsearch.

## Suggested execution order

    1. freeze baseline + classify all outbound client paths
    2. prototype Eggress BoxStream through eggfetch Dialer
    3. resolve the resolved-address/SSRF gate
    4. add feature-gated EggressDialer and typed error mapping
    5. add minimal secret-indirected route configuration
    6. centralize route-aware eggfetch client construction
    7. apply route only to qualified consumers
    8. add deterministic HTTP/SOCKS/multi-hop/TLS/pooling regressions
    9. add SSRF/redirect/pinning regressions for every dynamic routed path
    10. run dependency/footprint/performance characterization
    11. choose and document packaging Outcome A or B
    12. run local + exact-MSRV + cross-target gates
    13. run exact-candidate qualification when required
    14. update maintained docs/ownership table
    15. append implementation record and close registry entry

## Acceptance criteria

Phase 25 is complete only when all applicable items are true:

1. eggfetch remains the sole HTTP and destination-TLS owner.
2. Eggress is consumed through the listener-free outbound API, not eggress-embed.
3. The integration uses published 1.0.8 crates with no Git/path override.
4. Default direct behavior remains unchanged when no Eggress route is configured.
5. HTTP and SOCKS5 proxy paths pass deterministic end-to-end tests.
6. At least one mixed two-hop chain passes through the real adapter.
7. Eggfetch connection reuse above the Dialer is proven.
8. Timeout and cancellation behavior remain bounded without a competing outer
   timeout policy.
9. Typed Eggress failures are mapped without parsing display strings.
10. Proxy credentials remain absent from persisted config, Display/Debug, logs,
    and destination headers.
11. A failed configured chain never falls back to direct.
12. OriginController remains the application retry/circuit authority.
13. Phase 23 shared/widened client timeout semantics remain intact.
14. web_fetch and any equivalent dynamic target retain resolved-address/SSRF
    protection at least as strong as the pre-Eggress baseline, or remain
    explicitly excluded from Eggress routing.
15. Redirect targets remain revalidated before route establishment.
16. The default/no-feature dependency graph does not accidentally include
    Eggress.
17. The enabled feature graph contains only the justified Eggress base crates
    and protocols.
18. No SSH/russh, QUIC/H3, UDP, pproxy-legacy, legacy crypto, or insecure TLS
    stack enters the graph.
19. The binary/dependency/startup/performance delta is measured and explained.
20. One explicit default/release packaging outcome is recorded; no second SKU is
    created.
21. Rust 1.89 and the maintained target matrix compile.
22. make check, docs, packaging, release, and publish-dry-run gates pass as
    applicable.
23. Exact-candidate release qualification is rerun whenever the canonical
    release binary graph changes.
24. Current architecture/config/operator docs match the implemented traffic
    scope and failure semantics.
25. The implementation record and planning registry are closed with exact
    evidence rather than inferred success.

## Handoff notes

The attractive part of this integration is the narrow stream seam: Eggress
should be almost invisible above eggfetch. Resist introducing an Eggress service
inside eggsearch or teaching providers about proxy protocols.

The most important risk is not protocol compatibility; it is accidentally
bypassing eggsearch's existing DNS-resolution pinning on untrusted fetch
targets. Resolve that first. A narrower route that preserves security is better
than a global route that weakens the current SSRF contract.
