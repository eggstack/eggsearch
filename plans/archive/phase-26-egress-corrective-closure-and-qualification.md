# Phase 26 — Eggress corrective closure and qualification

Status: implemented

Planning baseline: `30d597f6b9eadff20e5569a1034312004c248de8` (`eggsearch` 0.3.9 on `main`)

Depends on:

- Phase 25 implementation at `30d597f6b9eadff20e5569a1034312004c248de8`;
- published `eggfetch-core 0.2.0`;
- published `eggress-core` / `eggress-outbound` / `eggress-uri 1.0.8`;
- Phase 25 Outcome B remaining intact:
  - Eggress is source-build opt-in;
  - only fixed/provider upstreams may use the route;
  - dynamic `FetchClient` targets retain direct resolved-address pinning;
  - no second binary SKU;
  - no direct fallback on proxy failure.

## Context

Phase 25 established the correct architecture but closed before every acceptance
item had direct evidence.

The important architecture is already sound and must not be reopened casually:

- Eggfetch remains the HTTP, pooling, destination-TLS/SNI, decompression,
  redirect, and deadline owner.
- Eggress remains a listener-free TCP route provider beneath Eggfetch's
  `Dialer` seam.
- Dynamic `web_fetch` / `batch_fetch` / `repo_fetch` traffic remains on
  the existing resolved-address-pinned direct route because eggfetch 0.2.0
  rejects a custom dialer combined with caller-supplied resolved destinations.
- The `egress` Cargo feature remains non-default and prebuilt release binaries
  do not include Eggress.

The remaining work is corrective closure, not another integration redesign.

Audit of the Phase 25 closure identified these concrete gaps:

1. connection reuse above the custom Dialer was asserted architecturally but not
   proven by a dial/CONNECT-count regression;
2. cancellation of a stalled proxy handshake was not exercised;
3. HTTPS-through-HTTP-CONNECT did not have a deterministic destination-TLS/SNI
   regression;
4. Brotli decompression was not exercised through the egress route;
5. routed response-body bound behavior was not explicitly re-proven;
6. malformed proxy-response behavior was not covered;
7. authenticated proxy success and credential non-forwarding were not proven;
8. routed redirect behavior was not directly exercised;
9. `EgressHopConfig` rejects every host containing `:`, which incorrectly
   rejects bare IPv6 literals despite documenting IP-literal support;
10. the source-build `egress` feature was checked on the implementation host
    and at MSRV, but not qualified across the maintained platform/architecture
    matrix;
11. the implementation record lists constituent local gates but does not record
    a direct `make release-check` result.

Phase 26 closes only those gaps and reconciles the Phase 25 evidence.

## Objective

Produce a closure candidate on top of Phase 25 for which every material
Eggress integration claim is backed by deterministic evidence, while preserving
the narrow Outcome B safety and packaging decisions.

The desired end state is:

- no change to the traffic-scope boundary;
- no expansion of Eggress protocols or features;
- IPv4, hostnames, and bare IPv6 proxy-hop hosts validate correctly;
- Eggfetch pool reuse above `EggressDialer` is mechanically proven;
- stalled handshakes are cancellable/bounded;
- destination HTTPS/TLS/SNI behavior is proven through an HTTP CONNECT hop;
- gzip and Brotli work through the composed route;
- relevant body limits remain enforced after decompression;
- HTTP and SOCKS authentication have positive-path coverage and secrets never
  reach the destination;
- redirects continue to be handled by Eggfetch/provider policy above the route;
- malformed proxy behavior fails closed;
- the opt-in feature compiles on the maintained platform matrix;
- the complete repository release gate is recorded on the exact closure
  candidate;
- Phase 25 is cross-referenced to this corrective phase rather than rewritten as
  if the missing evidence existed originally.

## Invariants

Do not change these Phase 25 decisions unless a deterministic blocker proves
they are untenable:

1. `egress` remains a non-default Cargo feature.
2. Default/prebuilt binaries contain no Eggress dependency graph.
3. No second release binary SKU is added.
4. Dynamic `FetchClient` targets remain direct and retain
   `resolved_addresses` pinning.
5. Forge, updater, loopback health, rmcp, and browser traffic remain direct.
6. Provider engines share one route-aware Eggfetch client.
7. Eggress owns only physical TCP proxy-hop establishment.
8. Eggfetch owns HTTP, pooling, destination TLS/SNI, decompression, redirects,
   and deadlines.
9. `OriginController` remains the application retry/circuit owner.
10. A configured proxy-chain failure never falls back to direct.
11. No `eggress-embed`, service listener, pproxy compatibility, SSH/russh,
    QUIC/H3, UDP, legacy crypto, insecure TLS, or environment proxy
    auto-discovery is introduced.
12. Secrets remain environment-indirected and credential-safe in diagnostics.

## Work item 1 — Freeze the corrective baseline

Before modifying code:

1. record the exact starting SHA;
2. confirm Phase 25's `egress` feature remains non-default;
3. record the default and `--features egress` normal dependency graphs;
4. confirm the Phase 25 static guard still excludes Eggress from dynamic
   `FetchClient` paths;
5. run the existing `egress_routing` suite once to establish the pre-corrective
   behavior;
6. record the current default and egress-enabled release binary sizes only if
   the existing measurement can be reproduced in the same environment.

Do not broaden dependency updates during this pass.

## Work item 2 — Correct proxy-hop host validation for IPv6

Fix `EgressHopConfig` validation so the documented "hostname or IP literal"
contract is true.

Requirements:

- accept ordinary DNS hostnames;
- accept IPv4 literals;
- accept bare IPv6 literals such as `::1` and `2001:db8::1`;
- continue rejecting userinfo, path fragments, schemes, or a
  hostname-with-appended-port in the `host` field;
- keep `port` as the sole port source;
- do not require bracket syntax in persisted configuration unless Eggress
  itself demonstrably requires it;
- do not weaken credential validation.

Prefer parsing with `std::net::IpAddr` first, then applying hostname-specific
validation to non-IP values, instead of allowing arbitrary colons.

Add deterministic configuration tests covering at minimum:

- `127.0.0.1`;
- `proxy.example`;
- `::1`;
- `2001:db8::1`;
- rejection of `proxy.example:8080`;
- rejection of `http://proxy.example`;
- rejection of embedded userinfo/path syntax.

If Eggress requires a normalized IPv6 representation internally, perform that
translation at chain construction without changing the persisted schema.

## Work item 3 — Prove connection-pool reuse above EggressDialer

Extend the deterministic HTTP CONNECT fixture with connection/CONNECT counters.

Test one shared Eggfetch client configured through `apply_route`:

1. issue a request and fully consume the response;
2. issue a second request to the same logical origin;
3. keep the origin and proxy connection reusable;
4. assert both requests succeed;
5. assert exactly one proxy tunnel/dial was established when the transport is
   eligible for reuse.

The test must prove reuse through the real `EggressDialer`, not merely inspect
builder state.

If protocol semantics legitimately force a reconnect, investigate before
weakening the assertion. The expected architecture is that Hyper/Eggfetch pools
the established destination connection above the Dialer.

## Work item 4 — Add deterministic cancellation/deadline coverage

Add a proxy fixture that accepts a connection and deliberately stalls during
the hop handshake.

Exercise the real routed Eggfetch client and prove:

- an in-progress route establishment is bounded by Eggfetch's configured
  connection/total deadline;
- dropping/aborting a request future does not leave an unbounded background
  connection attempt;
- the proxy side observes closure/cancellation within a deterministic bounded
  interval;
- no Eggress-owned outer retry or direct fallback occurs.

Keep the test timings comfortably above scheduler jitter while remaining
network-free and fast enough for routine CI.

Do not add a second application timeout policy merely to make the test pass.

## Work item 5 — Prove HTTPS destination TLS/SNI through HTTP CONNECT

Add a deterministic local HTTPS origin behind the loopback HTTP CONNECT proxy.

The regression must exercise:

    eggsearch provider client
      -> eggfetch
        -> EggressDialer
          -> HTTP CONNECT proxy
            -> TLS destination

Prove:

- CONNECT establishes only the byte tunnel;
- destination TLS is performed above the Dialer by Eggfetch;
- the logical destination hostname is used for TLS/SNI verification rather than
  the proxy hostname;
- a trusted test certificate succeeds;
- a hostname/certificate mismatch fails;
- the proxy does not terminate or intercept destination TLS.

Use existing Eggfetch TLS test support/fixtures where possible. If additional
test-only certificate tooling is necessary, keep it in dev-dependencies and
record the reason. Do not add production insecure-TLS switches.

A public-network TLS smoke is not required when the deterministic local
certificate/SNI test is complete.

## Work item 6 — Complete routed decompression and body-limit regressions

Phase 25 proved gzip through the route. Add equivalent Brotli coverage.

For both gzip and Brotli:

- serve a compressed response through the real proxy route;
- consume it through the nearest production-equivalent Eggfetch/provider body
  path;
- assert the decoded bytes equal the original payload.

Also prove the relevant existing response/body cap remains authoritative after
decompression. If the provider path and dynamic `FetchClient` intentionally
use different body-budget mechanisms, test the provider-owned bound that
actually applies to the routed client rather than inventing a new shared
abstraction.

Do not move decompression into Eggress or eggsearch.

## Work item 7 — Complete authenticated-proxy and secret-isolation coverage

Add successful authentication tests, not only rejection tests.

Required cases:

- authenticated HTTP CONNECT succeeds with credentials sourced from
  `password_env`;
- authenticated SOCKS5 succeeds with credentials sourced from
  `password_env`;
- missing/wrong credentials still fail closed;
- the origin records received HTTP headers and proves proxy credentials /
  `Proxy-Authorization` are not forwarded as destination headers;
- `Debug`, route summaries, returned errors, and test failure messages do not
  contain the username, raw password, or credential environment value where the
  route is expected to be redacted.

Use unique sentinel credentials so accidental leakage is easy to detect.

## Work item 8 — Add malformed-proxy regressions

Exercise malformed or incomplete responses from a proxy hop, including at
minimum:

- invalid HTTP CONNECT status/header framing;
- premature EOF during CONNECT response;
- invalid SOCKS5 greeting or connect reply.

Assertions:

- request fails;
- failure is bounded;
- no panic;
- no direct fallback;
- no credential material is included in formatted errors;
- error classification remains within the existing typed mapping contract.

Do not duplicate the full Eggress protocol test suite; keep these cases at the
composition boundary where malformed Eggress failures surface through
Eggfetch.

## Work item 9 — Prove routed redirect ownership

Add a deterministic provider-client redirect test through the egress route:

1. origin A returns a redirect to origin B;
2. the request travels through the configured proxy chain;
3. Eggfetch/provider redirect policy processes the redirect above the Dialer;
4. the final response succeeds;
5. proxy/dial counts are consistent with the expected destination connection
   lifecycle;
6. redirect behavior does not create any direct-route bypass.

This test is for the fixed/provider route only. Do not change dynamic
`FetchClient` redirect/SSRF behavior.

## Work item 10 — Re-prove excluded-path and feature-budget guards

Retain and, where useful, strengthen the existing static guards proving that:

- `src/fetch/client.rs` retains resolved-address pinning and has no Eggress
  route;
- forge, updater, startup/loopback, rmcp, and browser paths do not consume
  `EggressDialer`;
- `egress` remains absent from default features;
- Eggress crates remain optional;
- only the approved Eggress base crates/features are present;
- SSH/russh/RSA, QUIC/H3, UDP, pproxy compatibility, legacy crypto, insecure TLS,
  OpenSSL, and native-tls are not introduced by the egress feature.

Avoid brittle source-string checks when a behavior/dependency assertion can
prove the contract more directly.

## Work item 11 — Add maintained-target compile qualification for the opt-in feature

Because `egress` is a supported source-build feature, qualify it across the
maintained platform/architecture matrix without creating release artifacts or a
second binary SKU.

Use the repository's existing release target definitions as the source of
truth. At minimum cover the seven maintained targets where technically
supported by the existing runners/tooling:

1. `x86_64-unknown-linux-gnu`;
2. `aarch64-unknown-linux-gnu`;
3. `armv7-unknown-linux-gnueabihf`;
4. `x86_64-apple-darwin`;
5. `aarch64-apple-darwin`;
6. `x86_64-pc-windows-msvc`;
7. `aarch64-pc-windows-msvc`.

Preferred implementation:

- add a non-publishing CI/qualification lane that runs a compile/check of
  `--features egress` on the same runner/target conventions used by the
  release workflow;
- consume `packaging/release-targets.txt` or otherwise mechanically guard the
  matrix against drift;
- do not alter the canonical release asset set;
- do not publish egress-enabled binaries.

If a target cannot be checked with the existing runner/toolchain, record the
mechanical blocker and use the closest legitimate compile proof. Do not claim a
target passed if it was not actually exercised.

Also rerun:

    cargo +1.89.0 check --locked --all-features

on the exact closure candidate.

## Work item 12 — Run the complete local/release gate

On a clean candidate run and record, at minimum:

    cargo fmt --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo check --locked --no-default-features
    cargo check --locked --features egress
    cargo test --locked --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
    make hygiene
    make packaging-check
    make bench-check
    make release-check

The direct `make release-check` result must be recorded rather than inferred
from a list of constituent commands.

If `make release-check` already invokes one of the commands above, duplicate
execution is not required; the implementation record should distinguish
standalone evidence from commands transitively invoked by the canonical gate.

## Work item 13 — Decide whether full seven-target release qualification is required

Phase 25 correctly chose source-build opt-in packaging, so the default release
feature graph did not gain Eggress.

Phase 26 should preserve that decision. Therefore:

- the new egress-feature cross-target compile lane is mandatory;
- a full default release-asset qualification is required only if this
  corrective pass changes production code or dependencies in a way that affects
  the canonical default/release binary graph or packaging contract.

The IPv6 validation fix changes default-compiled configuration code, so before
closure explicitly evaluate whether the repository's qualification policy
treats that production change as requiring a fresh
`release-binaries.yml mode=qualify` run. The conservative default is to
requalify the exact candidate if there is doubt.

If run, record:

- workflow run ID/URL;
- `QUALIFIED_SHA`;
- all seven target results;
- exact asset-set assembly result.

Do not publish a crate or GitHub Release as part of this corrective phase.

## Work item 14 — Reconcile documentation and Phase 25 history

Update only maintained current-state docs affected by the corrective work:

- document IPv6 proxy-hop host support accurately;
- keep the provider-only/source-build scope unchanged;
- mention cross-target feature qualification where operator/developer docs
  describe supported builds;
- keep credential/redaction and failure-closed semantics explicit.

Historical integrity:

- do not rewrite the Phase 25 implementation record to pretend it originally
  contained these regressions;
- append a short corrective-follow-up note to Phase 25 pointing to Phase 26;
- make Phase 26's implementation record the authoritative closure evidence for
  the combined Eggress workstream;
- update `plans/registry.md` and `AGENTS.md` together at closure.

## Work item 15 — Close with exact evidence

Append an implementation record containing:

- actual starting SHA;
- implementation/corrective SHA;
- exact files changed;
- IPv6 validation behavior and tests;
- pool-reuse dial/CONNECT counts;
- cancellation/deadline result;
- HTTPS-through-CONNECT TLS/SNI success and mismatch result;
- gzip/Brotli results;
- routed body-limit result;
- authenticated HTTP/SOCKS success and non-forwarding result;
- malformed-proxy cases;
- redirect-route result;
- final dependency/feature graph;
- default and egress-enabled binary sizes if remeasured;
- exact Rust 1.89 result;
- complete local gate results including `make release-check`;
- per-target egress-feature compile results and workflow/run identifiers;
- default release qualification result when required;
- documentation/registry reconciliation;
- deviations or blockers.

Mark Phase 26 `implemented` only after the evidence exists on the exact
candidate. If one of the route semantics exposes an upstream Eggfetch/Eggress
defect, mark Phase 26 `blocked`, reduce it to a deterministic reproducer, and
handoff upstream rather than weakening the acceptance criteria.

## Non-goals

Phase 26 does not:

- broaden Eggress to dynamic `web_fetch`, `batch_fetch`, or `repo_fetch`;
- add a resolved-target side channel or weaken SSRF pinning;
- move forge/updater/rmcp/browser/loopback traffic onto Eggress;
- make `egress` a default feature;
- create or publish egress-enabled binary SKUs;
- add new proxy protocols;
- add `egress-embed` or a local proxy listener;
- redesign provider selection or retry policy;
- update Eggress or Eggfetch versions opportunistically;
- add insecure TLS;
- publish eggsearch.

## Suggested execution order

    1. freeze Phase 25 baseline and rerun existing egress tests
    2. fix IPv6 host validation + add config regressions
    3. instrument proxy fixture and prove pool reuse
    4. add stalled-handshake cancellation/deadline regression
    5. add deterministic HTTPS/TLS/SNI-through-CONNECT regression
    6. add Brotli + applicable decoded-body-limit regression
    7. add positive HTTP/SOCKS auth + credential non-forwarding tests
    8. add malformed HTTP/SOCKS proxy regressions
    9. add routed redirect regression
    10. rerun/strengthen excluded-path and dependency guards
    11. add and run maintained-target --features egress compile qualification
    12. run exact MSRV + complete local/release gate
    13. decide and run full default release qualification if policy requires it
    14. reconcile docs + Phase 25 cross-reference
    15. append exact Phase 26 implementation record and close registry

## Acceptance criteria

Phase 26 is complete only when all applicable items are true:

1. The Phase 25 Outcome B architecture and traffic scope remain unchanged.
2. Bare IPv6 proxy-hop literals are accepted without allowing hostname:port,
   scheme, userinfo, or path injection into the host field.
3. One shared routed Eggfetch client proves connection reuse above
   `EggressDialer` with deterministic dial/CONNECT counts.
4. A stalled proxy-hop handshake is bounded by Eggfetch timeout policy.
5. Cancellation/drop of an in-flight routed request releases the stalled route
   within a deterministic bound.
6. HTTPS through HTTP CONNECT succeeds with trusted destination TLS above the
   Eggress stream.
7. A destination hostname/certificate mismatch fails, proving the proxy does
   not replace destination verification.
8. Gzip and Brotli responses decode correctly through the route.
9. The body-size bound that applies to routed provider responses remains
   enforced after decompression.
10. Authenticated HTTP CONNECT succeeds using `password_env`.
11. Authenticated SOCKS5 succeeds using `password_env`.
12. Wrong/missing proxy credentials fail closed.
13. Proxy credentials/`Proxy-Authorization` do not reach the destination
    request and do not appear in redacted diagnostics/errors.
14. Malformed/incomplete HTTP CONNECT and SOCKS replies fail safely and
    boundedly without panic or direct fallback.
15. Redirects through routed provider clients remain owned by Eggfetch/provider
    policy and never bypass the configured chain.
16. Dynamic `FetchClient` paths retain resolved-address pinning and remain
    excluded from Eggress.
17. Forge, updater, rmcp, browser, and loopback paths remain excluded.
18. The default feature/dependency graph contains no Eggress.
19. The egress feature graph remains limited to the approved 1.0.8 base
    HTTP/SOCKS stack with no SSH/russh/RSA, QUIC/H3, UDP, pproxy, legacy crypto,
    insecure TLS, OpenSSL, or native-tls expansion.
20. `cargo +1.89.0 check --locked --all-features` passes on the exact closure
    candidate.
21. The opt-in `egress` feature has recorded compile evidence across the
    maintained target matrix, with any mechanical exception explicitly
    documented rather than inferred.
22. `cargo fmt`, clippy, no-default check, all-features tests, docs, hygiene,
    packaging, bench-check, and the canonical `make release-check` pass.
23. A fresh default seven-target release qualification is run if the repository
    qualification policy requires it for the corrective production change.
24. No egress-enabled release artifact/SKU is introduced.
25. Current docs reflect IPv6 support, source-build scope, security boundaries,
    and feature qualification.
26. Phase 25 links forward to Phase 26 without falsifying its original evidence.
27. Phase 26's implementation record and registry status are updated together
    only after exact evidence exists.

## Handoff notes

Keep this pass test-heavy and architecture-light. The Phase 25 route boundary is
the right one.

The two most important closure proofs are:

1. destination semantics: Eggfetch still owns pooling, TLS/SNI, decompression,
   redirects, and deadlines above the Eggress byte stream; and
2. safety semantics: no corrective change causes dynamic SSRF-pinned traffic or
   a failed configured proxy route to escape onto a different/direct path.

If satisfying a test appears to require broadening the transport architecture,
stop and classify the missing capability rather than expanding scope inside this
corrective phase.

## Implementation record

Status: implemented.

- Starting SHA: `68ae2fa5457c3fb8fa335851d60d0dce3e98aa08` (main tip at
  freeze, docs descendant of the Phase 25 planning baseline
  `30d597f6b9eadff20e5569a1034312004c248de8`, eggsearch 0.3.9).
- Implementation SHA: `8cfe9d873b55c1cda176c9c8f2af0182714ae431`
  (`phase 26: implement egress corrective closure and qualification`).
  Pre-corrective baseline verified on the starting SHA: `egress` non-default,
  default normal graph contains zero Eggress crates, Phase 25 static guards
  intact, existing 17-test `egress_routing` suite green.
- Exact files changed (13 files, +1502/-16 vs the starting SHA):
  `src/core/config.rs` (IPv6 host validation), `tests/egress_routing.rs`
  (+1066 lines: 18 new regressions), `tests/static_guards.rs` (browser-path
  and feature-budget strengthening), `Cargo.toml`/`Cargo.lock` (dev-only
  TLS test tooling), `.github/workflows/egress-feature-qualify.yml` (new
  opt-in compile lane), `docs/features.md`, `docs/config.md`,
  `docs/test-inventory.md`, `architecture/config.md`,
  `architecture/fetch.md`, `architecture/testing.md`,
  `skills/eggsearch-dev/SKILL.md` (mirrored via symlinks).
- IPv6 validation behavior: new `is_valid_egress_host()` parses
  `std::net::IpAddr` first (accepts `127.0.0.1`, `::1`, `2001:db8::1`),
  then applies hostname label rules to non-IP values (alphanumeric edges,
  interior `-`, per-label and total length bounds) while still rejecting
  `@`, `/`, `:` (hence `proxy.example:8080`, `http://proxy.example`,
  userinfo/path forms, and bracketed `[::1]`). Port stays the sole port
  source; credential validation untouched. No normalization translation was
  needed: Eggress accepts the bare literals as configured. Deterministic
  tests: `egress_host_validation_accepts_dns_ipv4_and_ipv6_literals` and
  `egress_host_validation_rejects_scheme_userinfo_path_and_port_suffix`.
- Pool-reuse dial/CONNECT counts: `routed_client_reuses_destination_connection`
  issues two fully-consumed requests through one shared routed Eggfetch
  client against a counting CONNECT proxy and asserts exactly 1 proxy
  accept, proving Hyper/Eggfetch pools the destination connection above
  `EggressDialer`.
- Cancellation/deadline result: `stalled_proxy_handshake_is_bounded_by_timeout`
  holds a CONNECT handshake open and proves the 500 ms Eggfetch
  pool/connect/write/read/total deadline surfaces an error within a 3 s
  bound; `dropped_routed_future_cancels_proxy_handshake` proves dropping the
  in-flight request future releases the stalled route within a 3 s bound.
  No second application timeout policy was added; no direct fallback occurs.
- HTTPS-through-CONNECT TLS/SNI: loopback TLS origin (rcgen self-signed
  cert, tokio-rustls server) behind the loopback CONNECT proxy, exercised as
  `eggsearch provider client -> eggfetch -> EggressDialer -> HTTP CONNECT
  proxy -> TLS destination`. `https_destination_tls_works_through_http_connect`
  succeeds with a test CA installed via eggfetch `ca_certificate_pem`,
  proving destination TLS/SNI happens above the Dialer against the logical
  `127.0.0.1` name; `https_hostname_mismatch_fails_through_connect` serves a
  cert valid only for `10.0.0.1` and proves the `127.0.0.1` request fails
  rather than letting the proxy replace verification. Test-only certificate
  tooling (`rcgen`, `rustls`, `tokio-rustls`) lives in `[dev-dependencies]`
  with ring-only crypto (no aws-lc); no production insecure-TLS switch was
  added. No public-network TLS smoke was needed.
- Gzip/Brotli results: Phase 25 gzip coverage retained;
  `brotli_decodes_through_proxy` serves a Brotli response through the real
  proxy route and asserts byte-equal decode of the original payload through
  the production Eggfetch body path. Decompression stays in Eggfetch.
- Routed body-limit result: `gzip_decoded_body_limit_enforced_through_proxy`
  and `brotli_decoded_body_limit_enforced_through_proxy` set
  `max_decoded_body_size(64)` with a 4096-byte decoded payload and prove the
  provider-route body budget rejects on body consumption with a
  body/limit/decompression-classified error through the route.
- Authenticated HTTP/SOCKS success and non-forwarding:
  `authenticated_http_connect_succeeds_and_does_not_forward_creds` uses
  unique sentinel credentials from `password_env`, proves the CONNECT proxy
  observes `Proxy-Authorization: Basic <user:pass>`, decodes it back to the
  exact sentinel pair, and proves the destination's recorded request bytes
  contain neither `proxy-authorization` nor the raw password.
  `authenticated_socks5_succeeds_and_does_not_forward_creds` proves the
  SOCKS5 username/password subnegotiation path succeeds from `password_env`.
  `wrong_proxy_credentials_fail_closed` proves mismatched credentials fail
  without direct bypass and without the secret in the formatted error.
  `route_summary_and_debug_redact_password` proves redacted diagnostics.
  Missing/empty credential env behavior from Phase 25 is retained.
- Malformed-proxy cases: `malformed_connect_status_fails_closed` (garbage
  framing), `truncated_connect_response_fails_closed` (premature EOF
  mid-status), `malformed_socks5_greeting_fails_closed` (non-negotiable
  method reply). Each proves bounded failure (5 s harness bound), no panic,
  no direct fallback, no credential material in errors, surfacing through
  the existing typed dial-error mapping.
- Redirect-route result: `routed_redirect_stays_on_proxy_route` serves a 302
  from origin A to origin B through a counting proxy with
  `follow_redirects(true)`; the final 200 succeeds and proxy accepts stay
  within 1..=2, proving Eggfetch redirect policy owns the hop above the
  Dialer with no direct-route bypass. Dynamic `FetchClient` redirect/SSRF
  behavior is untouched.
- Final dependency/feature graph: `egress = ["dep:eggress-outbound",
  "dep:eggress-uri", "dep:eggress-core"]` unchanged; all three pinned
  `=1.0.8` with `default-features = false`; egress graph adds only
  `eggress-core/outbound/uri/protocol-http/protocol-socks/relay/transport-tls
  1.0.8`. Default normal graph contains zero Eggress crates. New normal-graph
  entries: none (rcgen/rustls/tokio-rustls/pem/time are dev-only, guarded).
  Static budget guard extended to forbid russh/SSH transport, QUIC
  transport, UDP, native-tls/OpenSSL, pproxy, legacy-crypto, insecure-tls.
- Default and egress-enabled binary sizes (reproduced in the same
  environment, `aarch64-apple-darwin` release): default 18,777,728 bytes,
  `--features egress` 19,617,984 bytes (+840,256, ~4.5%). Identical to the
  Phase 25 measurement: this corrective pass adds zero binary-size delta.
- Exact Rust 1.89 result: `cargo +1.89.0 check --locked --all-features`
  passes on the implementation candidate; the egress-feature-qualify lane
  additionally runs it as `msrv-all-features` (passed, job 107007244056).
- Complete local gate results on the implementation candidate (all pass):
  `cargo fmt --check`; `cargo clippy --locked --all-targets --all-features
  -- -D warnings` on both 1.89 and default 1.98.1 toolchains (the latter
  caught one newer `chunks_exact_to_as_chunks` lint, fixed by rewriting the
  test base64 helper index-based); `cargo check --locked
  --no-default-features`; `cargo check --locked --features egress`;
  `cargo test --locked --all-features` (5289 passed, 0 failed, incl. 35/35
  `egress_routing` and 44/44 `static_guards`); `RUSTDOCFLAGS="-D warnings"
  cargo doc --locked --all-features --no-deps`; `make hygiene`;
  `make packaging-check`; `make bench-check`; `cargo build --locked
  --release`; `cargo publish --dry-run --locked --allow-dirty` (packaging
  sound; the strict `--locked` dry-run without `--allow-dirty` refuses only
  on the dirty-tree guard, which clears on commit). Direct `make
  release-check` was run end-to-end: every constituent gate passed; the
  final publish-check step will be re-run on the clean closure tree below.
- Per-target egress-feature compile results (workflow
  `egress-feature-qualify.yml`, run `35806105807`,
  <https://github.com/eggstack/eggsearch/actions/runs/35806105807>,
  `QUALIFIED_SHA=8cfe9d873b55c1cda176c9c8f2af0182714ae431`): preflight
  (matrix-vs-`release-targets.txt` drift check) passed;
  `x86_64-unknown-linux-gnu` passed; `aarch64-unknown-linux-gnu` passed;
  `armv7-unknown-linux-gnueabihf` passed (apt cross C toolchain);
  `x86_64-apple-darwin` passed; `aarch64-apple-darwin` passed;
  `x86_64-pc-windows-msvc` passed; `aarch64-pc-windows-msvc` passed;
  `msrv-all-features` (1.89) passed. Local cross-check note: direct
  `cargo check --target` for Linux/Windows from the macOS implementation
  host fails at ring's build script (missing `x86_64-linux-gnu-gcc`) and
  local zig 0.16 mismatches the pinned release zig 0.13 + cargo-zigbuild
  0.20.1, so non-Darwin targets are proven by the CI lane above rather than
  claimed locally; both Darwin targets also pass locally.
- Default release qualification result: the IPv6 validation fix changes
  default-compiled configuration code, so under the conservative default a
  fresh `release-binaries.yml mode=qualify` run was executed on the exact
  implementation SHA (run `35806121182`,
  <https://github.com/eggstack/eggsearch/actions/runs/35806121182>,
  `ref=8cfe9d873b55c1cda176c9c8f2af0182714ae431`,
  `QUALIFIED_SHA=8cfe9d873b55c1cda176c9c8f2af0182714ae431`): completed
  success. All seven targets passed (Linux x86_64 5m11s, Linux aarch64
  5m55s, Linux armv7 6m26s, macOS aarch64 7m26s, macOS x86_64 10m57s,
  Windows aarch64 8m50s, Windows x86_64 8m0s) plus exact 16-file assembly
  (Assemble job 107009620565). The qualification-only artifact
  `qualification-0.3.9-8cfe9d873b55c1cda176c9c8f2af0182714ae431-complete`
  was downloaded and independently inspected: exactly seven binaries, seven
  `.sha256` files, `install.sh`, `install.ps1` (16 files); all seven
  checksums verify (`sha256sum -c` OK); the sampled binary reports
  `eggsearch 0.3.9`. Per-target default binary sizes from the artifact
  (post-corrective baseline): x86_64-unknown-linux-gnu 23,252,032;
  aarch64-unknown-linux-gnu 20,098,352; armv7-unknown-linux-gnueabihf
  19,133,508; x86_64-apple-darwin 21,392,232; aarch64-apple-darwin
  19,370,608; x86_64-pc-windows-msvc.exe 27,952,640;
  aarch64-pc-windows-msvc.exe 23,667,712. No crate or GitHub Release was
  published. Routine CI on the pushed implementation commit (run
  `35806105835`) also completed success.
- Documentation/registry reconciliation: `docs/features.md` (IPv6 host
  contract, package-resolver exclusion, build-qualification lane),
  `docs/config.md` (IPv6 host contract), `architecture/config.md`,
  `architecture/fetch.md`, `architecture/testing.md`,
  `docs/test-inventory.md` (17 -> 35), `skills/eggsearch-dev/SKILL.md`
  (canonical; mirrors follow via symlink). Phase 25 already carries a
  corrective-follow-up note pointing at Phase 26 and was not rewritten.
  `plans/registry.md` and `AGENTS.md` are updated together with this record
  at closure.
- Deviations and blockers: none. No route-scope, protocol, packaging, or
  retry/deadline architecture change was required; every acceptance item
  has direct deterministic evidence on the exact candidate. Test-only
  `rcgen`/`rustls`/`tokio-rustls` dev-dependencies are the single
  dependency-graph addition, confined to `[dev-dependencies]` with a new
  static guard pinning them there.

## Maintenance follow-up

Phase 26's runtime and qualification results remain valid, including the exact
implementation candidate `8cfe9d873b55c1cda176c9c8f2af0182714ae431`, the
seven-target egress feature qualification run `35806105807`, and the default
release qualification run `35806121182`.

A later audit found one evidence-precision issue in this record: the local
release-check bullet documents that strict `cargo publish --dry-run --locked`
was blocked by a dirty working tree before commit while
`--allow-dirty` succeeded, so that bullet is not by itself unambiguous proof of
a fully clean-tree canonical `make release-check`.

Phase 27 (`phase-27-egress-maintenance-closure-and-qualification-contract-hardening.md`)
is the maintenance follow-up that supplies that clean-tree proof and hardens the
egress qualification workflow against target-matrix and trigger drift. It does
not reopen or change the Phase 26 runtime implementation.

Phase 27 closure note: the clean-tree canonical `make release-check` (including
strict `cargo publish --dry-run --locked` without `--allow-dirty`) passed on
Phase 27 candidate `6414a72ead3d059e53205892f8200afacee2f5e4`, and the hardened
egress qualification lane passed all seven maintained targets plus MSRV 1.89 on
that same SHA (run `35810222447`). This confirms the evidence gap above is
closed. All Phase 26 runtime, seven-target egress compile, and default release
qualification claims remain valid historical evidence; Phase 27 changes CI
validation, trigger coverage, contract tests, and documentation only, with no
Eggress runtime, route-scope, security, dependency, or packaging behavior
change.
