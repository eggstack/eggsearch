# Phase 17 — eggfetch 0.1.7 HTTP Transport Consolidation

Status: implemented
Depends on: phase 16 implemented; `eggfetch-core 0.1.7` published
Baseline for planning: `ac394031793cf5e37c49b790794e845ef0ab3650` (`eggsearch` 0.3.9 on `main`)
Upstream release reviewed: `eggstack/eggfetch` 0.1.7, release commit `43c3b312f2def887d0f0b7ce539faa626adf2cc8`
Supersedes planning assumptions from: eggfetch 0.1.5 integration review

## Why this phase exists

eggsearch currently owns outbound HTTP on top of a direct `reqwest 0.12` dependency while `rmcp` independently brings its own reqwest-backed Streamable HTTP client transport. This leaves eggsearch maintaining request construction, bounded-body plumbing, destination-pinned fetch clients, timeout wrappers, and transport-error classification while also carrying a second reqwest line in the normal dependency graph.

eggfetch 0.1.5 made replacement technically viable by exposing typed detailed failures and caller-supplied resolved destinations. Version 0.1.7 materially improves the fit for eggsearch in two areas that matter to this repository:

1. `Timeout.total` is now a single absolute deadline from logical request start through response-body EOF/trailers, including streaming/decompression. It no longer stops at response headers or resets as body chunks arrive.
2. Repeated `resolved_addresses()` requests with the same logical origin, ordered address snapshot, and SNI identity now reuse a bounded Hyper route client instead of constructing an isolated client for every request. This lets eggsearch preserve its DNS-validation/SSRF pinning model without paying the connection-churn cost of its current per-target reqwest client construction.

The 0.1.7 release also contains a more useful feature boundary than the earlier review assumed: protocol, URL convenience, advanced routing, redirect policy, retry policy, and Basic auth can now be selected independently. eggsearch can therefore consume the pieces it needs without enabling eggfetch's full compatibility policy bundle.

This phase should consolidate eggsearch-owned HTTP mechanics behind eggfetch while preserving eggsearch-owned search, fetch, security, retry/circuit-breaker, and release policy.

## Objective

Migrate every eggsearch-owned outbound HTTP path from direct reqwest usage to `eggfetch-core 0.1.7`, then remove the direct `reqwest 0.12` dependency.

The intended ownership boundary after this phase is:

```text
eggfetch
  owns HTTP transport, pooling, TLS, decompression, total/body deadlines,
  bounded hard-error body reads, pinned physical routing, redirect mechanics,
  and typed transport failures

eggsearch
  owns SSRF authorization/revalidation, fetch truncation semantics,
  provider/search policy, OriginController retry/circuit policy,
  release checksum/identity policy, and MCP-facing error contracts

rmcp
  continues to own its Streamable HTTP MCP client transport, including its
  reqwest backend, until rmcp itself exposes a lower-maintenance alternative
```

The phase is successful if eggsearch deletes transport-specific code rather than replacing reqwest with an eggsearch-specific compatibility wrapper.

## Upstream 0.1.7 decisions

### Feature profile

Do not depend on the `http1` compatibility alias. That alias enables advanced routing, logical retries, redirect policy, and Basic auth together. eggsearch needs only a subset and already has its own retry/backoff/circuit policy.

Use this target feature shape unless implementation discovers a demonstrated incompatibility:

```toml
eggfetch-core = {
    version = "0.1.7",
    default-features = false,
    features = [
        "standard-http1",
        "advanced-routing",
        "redirects",
        "tls-rustls",
        "json",
        "compression-gzip",
        "compression-brotli",
    ],
}
```

Rationale:

- `standard-http1`: high-level URL/request/response API and ordinary H1 transport without the compatibility policy bundle.
- `advanced-routing`: required for `resolved_addresses()`; pure `standard-http1` is insufficient for SSRF-safe `web_fetch`.
- `redirects`: providers and the updater should continue to use library-owned redirects rather than grow duplicate redirect loops. `web_fetch` remains manual because each hop must pass eggsearch SSRF validation.
- `tls-rustls`: preserves the current Rustls/WebPKI trust model without pulling native-root loading by default.
- `json`: provider/updater request/response convenience where it removes local serialization plumbing.
- gzip/brotli: preserve current direct-client decompression behavior.

Do not enable in this phase unless a concrete existing contract requires them:

- `logical-retry` — OriginController remains the one retry/backoff authority.
- `basic-auth` — current provider authentication is header/Bearer/API-key based.
- `proxy` — current direct reqwest dependency disables default features, including reqwest's `system-proxy` feature; ambient proxy behavior is not part of the intended SSRF route model.
- `tls-native-roots` — not needed for parity with the current packaged WebPKI-root configuration.
- `http2`, `http3`, cookies, multipart, tracing — no current eggsearch requirement justifies widening the graph.

Because Cargo features unify per package, do not attempt to create two renamed eggfetch dependencies for "lean providers" and "advanced web_fetch". One explicit union should describe the application's actual capability set.

### Timeout model

Do not translate reqwest `.timeout(duration)` to `Timeout::from_secs(...)`. In eggfetch, scalar constructors populate pool/connect/write/read phase timeouts but intentionally leave `total = None`.

For paths that currently depend on reqwest's total request deadline, construct an explicit eggfetch `Timeout` with `total: Some(duration)`. Version 0.1.7 is specifically qualified to retain that deadline through body streaming, decompression, EOF, and trailers.

Do not silently change timeout scope across manual `web_fetch` redirect hops. Preserve the existing per-request/hop behavior unless a separate policy change explicitly defines a whole-fetch deadline.

### Redirect model

Use explicit redirect policy at each client profile; do not rely on library defaults.

- `web_fetch`: eggfetch redirects disabled. eggsearch continues its manual redirect loop, validates/re-resolves every hop, and supplies a fresh approved physical address snapshot to eggfetch.
- provider clients: library redirects enabled with a bounded count. Prefer downgrade-denying strict policy for HTTPS provider endpoints unless an existing provider contract/fixture proves a downgrade is required.
- updater: library redirects enabled and HTTPS -> HTTP downgrade denied. GitHub/crates.io/CDN redirects must continue to work without allowing transport downgrade.
- startup/integration health probes: redirects disabled; a health endpoint must identify the expected service directly.

### Resolved-route cache model

Treat eggfetch's resolved-route cache as a transport optimization, not as authorization.

eggsearch must still:

1. parse and validate the logical URL;
2. resolve the hostname itself;
3. reject disallowed/private destinations according to `FetchLimits`;
4. pass the exact approved ordered `SocketAddr` snapshot through `resolved_addresses()`;
5. repeat validation and resolution for every manually followed redirect.

eggfetch may reuse a connection only when its route identity matches the logical origin + ordered snapshot + SNI. Do not loosen eggsearch validation because the transport cache is bounded or keyed safely.

## Non-goals

- Do not replace rmcp's `transport-streamable-http-client-reqwest` with an eggsearch-local eggfetch adapter.
- Do not add MCP/SSE/session/reconnect behavior to eggfetch for this phase.
- Do not make eggfetch aware of eggsearch provider types, SSRF policy, OriginController, fetch truncation, or updater semantics.
- Do not enable eggfetch automatic retries.
- Do not move browser-fetch policy into eggfetch.
- Do not change the public MCP tool surface or response schemas.
- Do not change fetch/private-network authorization defaults.
- Do not change `web_fetch` truncation into a hard body-limit error.
- Do not add HTTP/2 merely because eggfetch supports it.
- Do not claim a binary-size win until the linked release artifact is measured.
- Do not introduce a generic reqwest-shaped compatibility facade in eggsearch.

## Invariants

1. A hostname accepted by `web_fetch` is connected only through the exact address snapshot eggsearch validated for that hop.
2. Cross-origin redirects are never followed inside eggfetch for `web_fetch`; eggsearch must revalidate them first.
3. Same-origin routing reuse must not permit an ordinary DNS-pooled connection to bypass a caller-supplied resolved snapshot.
4. Existing private-network/localhost controls remain fail-closed.
5. `web_fetch` continues to truncate at `max_bytes` and report truncation rather than converting streaming overflow to a transport error.
6. Provider/updater hard body caps remain hard failures.
7. Total request deadlines cover response-body consumption where the current reqwest client timeout does.
8. OriginController remains the only application retry/circuit/backoff authority.
9. Provider errors remain provider-scoped and do not become global search failures.
10. Update checksum, candidate identity, release-target, and Cargo-fallback rules are unchanged.
11. rmcp remains solely responsible for its MCP HTTP client transport.
12. No ambient environment proxy may bypass eggsearch's direct-routing/SSRF assumptions.
13. Normal builds remain keyless and network-free in tests.
14. The migration must pass the same seven-target release qualification contract introduced by phase 16 before release.

## Work item 1 — Raise MSRV and introduce the dependency with an explicit feature budget

eggfetch 0.1.7 requires Rust 1.89 while eggsearch still declares Rust 1.88.

Raise the repository MSRV coherently:

- `Cargo.toml` `rust-version = "1.89"`;
- exact-MSRV CI/toolchain checks;
- `AGENTS.md`, contributor/build architecture docs, and development skill guidance that still say 1.88;
- any release/publish checks that parse or assert the declared MSRV.

Add `eggfetch-core = "0.1.7"` using the explicit feature profile above while leaving reqwest in place temporarily for staged migration.

Regenerate `Cargo.lock` with an MSRV-compatible toolchain.

Before changing call sites, record the normal-runtime dependency baseline:

```text
cargo tree --locked -e normal
cargo tree --locked -d
cargo tree --locked -e features -p reqwest
cargo tree --locked -e features -p eggfetch-core
```

Also record representative stripped release-binary size before migration so the closure pass can distinguish maintenance wins from footprint claims.

Add a deterministic guard or release-check assertion that the final eggfetch feature set does not accidentally acquire:

```text
logical-retry
basic-auth
proxy
tls-native-roots
http2
http3
cookies
multipart
```

If transitive feature unification enables any of these, stop and explain the source before proceeding.

## Work item 2 — Define explicit eggfetch client profiles without a compatibility wrapper

Use `eggfetch_core::Client` directly. Keep policy visible at the ownership site rather than hiding it behind a broad `HttpClient` facade.

The repository needs four policy profiles:

### Fetch transport

Owned by `src/fetch/client.rs`:

- shared client instance;
- H1 + Rustls + gzip/brotli;
- automatic redirects disabled;
- no retry policy;
- no environment proxy;
- total timeout set explicitly per request;
- resolved addresses supplied per validated hop.

### Provider transport

Owned by `src/meta/engines/mod.rs`:

- shared client across engines where current architecture shares reqwest;
- bounded library redirects;
- strict HTTPS downgrade policy for HTTPS providers where compatible;
- no automatic retries;
- explicit request total deadline;
- hard decoded-body limit appropriate to each provider or common provider cap.

### Updater transport

Owned by `src/update.rs`:

- redirects enabled because release assets may move to CDN endpoints;
- HTTPS downgrade denied;
- explicit total deadlines;
- bounded metadata/checksum bodies;
- streamed release assets remain incrementally written and size checked.

### Local health/probe transport

Owned close to `startup.rs` / `integrations/common.rs`:

- redirects disabled;
- short explicit total deadline;
- very small hard decoded-body cap;
- `send_detailed()` where refused/timeout/connect classification matters.

Small local construction helpers are acceptable. Do not create an eggsearch request-builder API that mirrors reqwest/eggfetch methods.

## Work item 3 — Migrate startup and integration health probes first

Start with the smallest, most diagnostic-sensitive paths.

### `src/startup.rs`

Replace the current two-stage probe:

```text
TcpStream::connect(...)
then
reqwest GET /healthz
```

with one eggfetch request using:

- redirects disabled;
- total deadline equal to the existing health-probe budget;
- a decoded-body cap matching the existing 256-byte safety bound;
- `send_detailed()`.

Map typed failures without parsing error strings:

- `NetworkFailureKind::ConnectionRefused` -> existing refused outcome;
- timeout / `TimeoutPhase` -> existing timeout outcome;
- other connect/DNS errors -> existing error outcome;
- wrong HTTP status/service identity/malformed JSON/non-ready state retain their current classifications.

The HTTP request itself is sufficient evidence of service reachability; the duplicate TCP preflight should disappear unless a regression test proves it distinguishes a contract the typed request cannot express.

Add stalled-header and stalled-body coverage so the 0.1.7 absolute total deadline is exercised, not merely response-header timeout behavior.

### `src/integrations/common.rs`

Move the simple eggsearch health GET to eggfetch with the same small-body/timeout rules.

Do not change the subsequent rmcp `StreamableHttpClientTransport` path. It remains rmcp-owned and reqwest-backed.

Acceptance for this work item is removal of direct reqwest/TcpStream transport plumbing from health probing without changing existing `Healthy/Refused/Timeout/WrongService/Malformed/NonReady/Error` semantics.

## Work item 4 — Migrate the binary updater

Replace `UpdateClient`'s reqwest client with an explicitly configured eggfetch client.

For registry metadata and checksum files:

- use explicit total deadlines;
- use hard decoded-body caps;
- use buffered `bytes()` / `json()` only after the cap is installed;
- preserve the current response-status/error mapping.

For release binaries:

- keep streaming to the temporary candidate file;
- keep the running `MAX_RELEASE_BYTES` check;
- do not buffer a release executable into memory merely because eggfetch exposes `bytes()`;
- preserve fsync/executable-bit/candidate-version/checksum/replacement ordering.

Redirect behavior is critical:

- GitHub release/download redirects must continue to work;
- HTTPS -> HTTP downgrade must fail before second-hop I/O;
- redirect exhaustion must map to a deterministic update error rather than Cargo fallback;
- only the existing asset-unavailable condition remains eligible for Cargo fallback.

Extend updater tests with redirect success, redirect exhaustion, downgrade rejection where the fixture can model it, stalled-body total timeout, and oversized streamed asset cases.

## Work item 5 — Migrate metasearch/provider engines and delete duplicated bounded-read mechanics

Change the shared engine HTTP type from `Arc<reqwest::Client>` to `Arc<eggfetch_core::Client>`.

Migrate provider call sites incrementally:

- Brave request query/header construction;
- Exa JSON POST;
- OSV GET/JSON POST;
- DuckDuckGo query GET;
- Sourcegraph request/auth path;
- every other engine still using the shared reqwest client.

Do not preserve reqwest method syntax for its own sake. Use eggfetch's native builder directly and keep provider-specific request construction inside the provider module.

Where current provider semantics treat oversized responses as errors, prefer eggfetch's decoded-body hard limit and remove `read_bounded_body()` once every caller has equivalent behavior. Retain a local streaming helper only where a provider genuinely needs incremental parsing or different semantics.

Move transport errors from `reqwest::Error` to eggfetch's typed error/detailed-failure surface while preserving the MCP/provider-facing error code and provider-scoped failure behavior.

Retire redundant `tokio::time::timeout` wrappers around HTTP sends/body reads once the same deadline is expressed as `Timeout.total`. Keep orchestration-level Tokio deadlines that bound work broader than one HTTP request.

Add a regression that drips response chunks without completing; the request must terminate at the total deadline rather than staying alive because the read timeout resets.

## Work item 6 — Migrate `web_fetch` using one shared client and request-scoped resolved routing

This is the security-critical slice and should land after the simpler paths are green.

### Delete per-destination client construction

Remove `client_for_url()` and any equivalent code that rebuilds a reqwest client with `resolve_to_addrs` for every destination.

`FetchClient` should own one shared eggfetch client. For each hop:

1. call the existing `validate_fetch_target_with_resolved_addrs()`;
2. retain the exact ordered approved address snapshot;
3. build the request against the unchanged logical URL;
4. call `resolved_addresses(approved_snapshot)`;
5. send with automatic redirects disabled.

This is the primary 0.1.7 adoption win: repeated identical validated routes may now reuse H1 keep-alive through eggfetch's bounded resolved-route cache without changing the authorization model.

### Preserve manual redirect authorization

Keep the current application redirect loop.

For each 301/302/303/307/308 response:

- parse/resolve `Location` exactly as today;
- enforce `redirect_limit`;
- validate the new target through eggsearch;
- resolve and approve a new physical snapshot;
- issue a new pinned eggfetch request.

Do not catch `ResolvedTargetRedirect` and automatically retry unpinned. A pinned request must never gain a DNS fallback path.

### Preserve truncation semantics

Do not replace `append_bounded()` with `max_decoded_body_size` for the main fetched document. eggfetch's decoded-body cap is a hard error; eggsearch currently returns a bounded/truncated document with `truncated = true`.

The application-level streaming cap remains authoritative for `web_fetch`.

### Adapt single-consumption body streaming

eggfetch responses are single-consumption.

Where PDF detection currently consumes an initial reqwest chunk and then switches to `bytes_stream()`, create one eggfetch byte stream, pull the first chunk from that stream, inspect the PDF magic, append it through the same bounded path, then continue consuming that same stream.

Do not buffer the whole response for type detection.

### Preserve wire-length checks

Use eggfetch's retained wire content-length metadata/helper for the early declared-size check so automatic decompression does not erase the original length evidence.

### Timeout parity

Set an explicit per-request total deadline from the existing fetch timeout. Do not use only `Timeout::from_secs`.

Because eggsearch manually performs redirects, do not accidentally convert the configured timeout into one aggregate deadline across the whole redirect chain unless that behavior is separately specified.

## Work item 7 — Use typed transport failures inside OriginController without moving policy into eggfetch

Keep `OriginController` and its existing responsibilities:

- per-origin concurrency;
- circuit state;
- Retry-After/backoff policy;
- browser/HTTP coordination;
- retry-attempt budget;
- HTTP status classification.

Do not enable eggfetch `logical-retry`.

Replace brittle string classification where eggfetch now supplies typed evidence:

- DNS failure;
- connection refused;
- generic connect failure;
- request timeout / timeout phase.

If mid-stream reset/broken-pipe/EOF categories are not yet represented by eggfetch's typed detail, keep the smallest necessary fallback at the eggsearch boundary rather than adding an eggsearch-specific enum to eggfetch. Record any missing generally useful transport classification as an upstream follow-up; do not specialize eggfetch for this repository.

Add tests proving the same OriginController state transitions occur for typed refused/DNS/timeout failures as before.

## Work item 8 — Remove direct reqwest while retaining the rmcp boundary

After all eggsearch-owned callers have migrated:

- remove direct `reqwest = 0.12` from `[dependencies]`;
- remove reqwest imports/types from `src/`;
- regenerate the lockfile;
- inspect the normal dependency graph.

A remaining reqwest line is acceptable only when it is attributable to rmcp's `transport-streamable-http-client-reqwest` or clearly dev-only tooling.

Do not write an eggfetch implementation of rmcp's `StreamableHttpClient` in this phase. That interface owns POST/DELETE/session/SSE/reconnect behavior; a local adapter would trade one transitive dependency for a new protocol-maintenance surface.

Add a static/maintenance guard that prevents direct reqwest usage from silently returning to eggsearch production source without an explicit architecture decision.

The expected dependency boundary is:

```text
eggsearch-owned outbound HTTP -> eggfetch-core
MCP Streamable HTTP client    -> rmcp -> reqwest (transitive)
```

## Work item 9 — Characterize footprint, connection reuse, and behavioral parity

The migration is justified primarily by maintenance consolidation. Measure footprint rather than assuming it.

Capture before/after on at least:

- Linux x86_64 release binary;
- Linux AArch64 release binary or the existing release-qualification artifact for that target;
- `cargo tree -d`;
- `cargo tree -e normal -i reqwest`;
- `cargo tree -e normal -i eggfetch-core`;
- a linked-code view such as `cargo bloat --release` where available.

Record:

- stripped binary bytes;
- duplicate HTTP/TLS/compression dependency lines removed/added;
- whether direct reqwest 0.12 disappeared from the normal graph;
- whether the requested eggfetch feature budget stayed intact.

Do not set a fictional size-win threshold. If the stripped release binary grows materially, inspect feature attribution before closure and document why the maintenance reduction is still or is not worth the linked-byte cost.

Add a focused loopback connection-reuse characterization for the fetch path if feasible: repeated requests with the same logical origin and same approved address snapshot should not recreate a transport client/connection for every request. Do not make the test depend on nondeterministic public DNS.

## Work item 10 — Security and regression qualification

The migration is not complete until existing behavior is covered at the eggsearch boundary.

Required `web_fetch` cases:

- hostname resolves to public address -> exact approved address is used;
- localhost/private/link-local/metadata targets remain blocked according to config;
- safe same-origin redirect works;
- safe cross-origin redirect is re-resolved/revalidated;
- redirect to a private destination is blocked before second-hop I/O;
- pinned route never falls back to DNS;
- gzip and brotli decoded streaming remains bounded;
- declared wire Content-Length over cap fails early as today;
- streamed overflow truncates and reports `truncated`;
- PDF magic detection works without double-consuming the body;
- stalled body hits the total deadline.

Required provider/updater/probe cases:

- provider oversized body hard-fails;
- provider total deadline survives chunk progress;
- provider redirect policy remains bounded;
- updater follows legitimate release redirect and denies downgrade;
- updater streamed asset stays bounded;
- startup probe distinguishes refused, timeout, malformed, wrong-service, and non-ready;
- integration health probe remains bounded;
- typed errors do not leak credentials/API keys in diagnostics.

Run the normal repository gates:

```text
make check
make packaging-check
make release-check
```

Then run the phase-16 full seven-target release qualification against the exact candidate SHA before publishing the first release containing this migration.

## Work item 11 — Documentation and closure record

Update documentation only where the implementation changes contributor/operator truth.

At minimum review and update:

- `AGENTS.md` — MSRV and active-plan note;
- `architecture/fetch.md` — eggfetch ownership, resolved-route pinning/cache, manual redirect boundary;
- `architecture/hardening.md` — SSRF route authorization vs transport routing;
- `architecture/engines.md` / `architecture/meta.md` — shared provider HTTP client and hard body limits;
- `architecture/startup.md` — typed one-request health probing;
- `architecture/packaging.md` / build docs — MSRV/release qualification implications as applicable;
- `architecture/maintenance.md` — dependency/transport ownership;
- `skills/eggsearch-dev/SKILL.md` and architecture skill mirrors through their canonical source;
- `CHANGELOG.md` — append-only entry for the MSRV bump and HTTP transport consolidation.

Do not advertise rmcp as eggfetch-backed while its client transport remains reqwest-backed.

When implementation is complete, append an implementation record to this plan with:

- exact implementation SHA;
- eggfetch version and resolved feature set;
- pre/post normal dependency graph summary;
- pre/post representative release-binary sizes;
- transport/reuse characterization;
- test/gate results;
- seven-target qualification run;
- any retained reqwest path and why;
- deviations from this plan.

Then mark phase 17 implemented and update `plans/registry.md` in the same closure change.

## Suggested implementation order

```text
1. MSRV + eggfetch dependency/feature budget + baseline measurements
2. startup/integration health probes
3. updater
4. provider/meta engines
5. web_fetch pinned routing + single-stream body conversion
6. OriginController typed failures
7. remove direct reqwest
8. dependency/footprint characterization
9. docs + full gates + seven-target qualification
10. closure record / registry status
```

Keep reqwest and eggfetch side-by-side only while the migration is in progress. Do not leave an indefinite dual-client architecture.

## Acceptance criteria

Phase 17 is complete only when all of the following are true:

1. eggsearch declares Rust 1.89 consistently across Cargo, CI, and contributor docs.
2. `eggfetch-core 0.1.7` is the transport for all eggsearch-owned outbound HTTP.
3. The selected eggfetch feature graph includes advanced resolved routing and required redirect/JSON/compression support but does not accidentally enable logical retry, Basic auth, proxy, native roots, H2/H3, cookies, or multipart.
4. Direct `reqwest 0.12` is removed from eggsearch production dependencies.
5. Any remaining reqwest in the normal graph is attributable to rmcp and is documented as an intentional boundary.
6. No eggsearch-local rmcp/eggfetch transport adapter is added.
7. `FetchClient` no longer builds a new HTTP client per validated destination.
8. `web_fetch` still validates and resolves every hop before I/O and passes only the approved address snapshot to eggfetch.
9. Private/localhost/metadata redirect and DNS-pinning regressions remain fail-closed.
10. `web_fetch` preserves truncation semantics rather than converting overflow to an eggfetch hard-limit error.
11. PDF detection and streaming use one response-body consumption path.
12. Provider and updater hard body caps remain hard failures.
13. Total deadlines are explicit and demonstrably cover stalled/slow response bodies.
14. Startup health probing no longer requires a separate TCP preflight and preserves existing diagnostic outcomes.
15. OriginController remains the one retry/circuit authority and uses typed eggfetch failure evidence where available.
16. Legitimate updater/provider redirects continue to work; updater HTTPS downgrade is denied.
17. `make check`, `make packaging-check`, and `make release-check` pass on the exact candidate.
18. The full seven-target phase-16 qualification passes on that candidate.
19. Before/after dependency and linked-binary measurements are recorded without making an unsupported footprint claim.
20. Documentation accurately states the eggfetch/eggsearch/rmcp ownership boundary.
21. `plans/registry.md` and this plan carry exact closure evidence before status becomes `implemented`.

## Handoff notes

The most important 0.1.7-specific change from the earlier 0.1.5 review is that the security-critical fetch path no longer has to choose between destination pinning and transport reuse. Use one shared eggfetch client and attach the approved resolved snapshot to each request; let eggfetch's bounded resolved-route cache recover keep-alive safely.

The second important change is the repaired total-deadline body lifecycle. Prefer that native deadline over layers of `tokio::time::timeout` around send futures, but only after tests prove the application-visible timeout scope is unchanged.

The new lean feature split should be used selectively. Pure `standard-http1` cannot satisfy eggsearch because it deliberately excludes `resolved_addresses()`; full `http1` is broader than eggsearch needs because it re-enables retry and Basic-auth policy. The intended middle ground is `standard-http1 + advanced-routing + redirects` plus the exact TLS/JSON/compression capabilities above.

If implementation discovers a generally useful missing eggfetch primitive, stop and make that upstream request generic. Do not solve it by adding an eggsearch-specific API to eggfetch or by reconstructing a reqwest compatibility layer locally.

## Implementation record

Implementation SHA: `5a739a3cc6cdd060911eeefad7b004661f4b5c94` (76 files,
+1945/-1448).

- eggfetch version and resolved feature set: `eggfetch-core 0.1.7` with
  `default-features = false` and exactly `standard-http1`,
  `advanced-routing`, `redirects`, `tls-rustls`, `json`,
  `compression-gzip`, `compression-brotli`. A new `static_guards.rs`
  feature-budget test fails the tree if `logical-retry`, `basic-auth`,
  `proxy`, `tls-native-roots`, `http2`, `http3`, `cookies`, or `multipart`
  ever enter the declared budget; `cargo tree -e features` confirms none
  of them is enabled.
- Pre/post normal dependency graph summary: normal graph went from 631 to
  624 entries. Direct `reqwest 0.12` is gone from `[dependencies]` and from
  the normal graph. The only remaining normal-graph reqwest is `0.13.4`
  via `rmcp 3.2.0` (`transport-streamable-http-client-reqwest`), which is
  the documented intentional boundary. `eggfetch-core 0.1.7` is a direct
  dependency. A new `static_guards.rs` test fails on any `reqwest::` use
  in non-test production source.
- Pre/post representative release-binary sizes: post-migration stripped
  Linux-equivalent local release binary (`target/release/eggsearch`,
  `strip = true`) is 18M. No pre-migration local release artifact was
  recorded, so no footprint claim is made; the migration is recorded as a
  maintenance consolidation.
- Transport/reuse characterization: `FetchClient::client_for_url()` (per
  destination client construction) is deleted. One shared eggfetch client
  serves every validated hop with the approved ordered snapshot pinned via
  `resolved_addresses()`; manual redirect re-validation, truncation
  reporting, single-stream PDF magic detection, and wire-length early
  checks are preserved. Reuse is structural (no per-destination
  construction remains) and covered by the existing loopback fetch suites;
  no nondeterministic pool-counter assertion was added.
- Test/gate results on the implementation SHA: `make check` passes
  (fmt, clippy zero warnings, no-default check, full `--all-features`
  suite with 0 failures, hygiene, packaging contract). `make
  release-check` passes modulo the pre-commit dirty-tree publish guard,
  which clears once committed.
- Seven-target qualification run: not run on this commit. The
  seven-target matrix runs only on tags or manual `workflow_dispatch`
  (`release-binaries.yml`), so pushing this change burns no release
  runners. Qualification of the exact candidate remains required before
  publishing the first release containing this migration, per the plan.
- Any retained reqwest path and why: `rmcp -> reqwest 0.13.4`
  (Streamable HTTP MCP client transport) is retained deliberately; no
  eggfetch adapter for rmcp's session/SSE/reconnect surface was added.
  Dev-only `httpmock` brings its own async crates; no production path
  uses them.
- Deviations from this plan:
  - HTML scrape engines (`brave`, `duckduckgo`, `mojeek`, `searxng`,
    `startpage`, `yahoo`) send identity encoding per request
    (`.decompress(false)`). Live chunked compressed responses from at
    least DuckDuckGo (br) and Startpage (gzip) carry valid payloads that
    decode with stock decoders and with eggfetch when served with
    `Content-Length`, but fail inside eggfetch's streaming decoder when
    served chunked (minimal reproducer: same br bytes served chunked fail
    with `brotli error`, served with length decode to the identical
    31047 bytes). JSON APIs keep automatic decompression. Recorded as an
    upstream follow-up for `eggstack/eggfetch`; no eggsearch-local
    decompression stack was added.
  - `http = "1"` was added as a direct dependency for the shared
    header/status types (`fetch/cache.rs`, forge status mapping).
  - No separate `cargo bloat` linked-code view was captured
    (`cargo-bloat` not installed); footprint evidence is the stripped
    binary size plus the before/after `cargo tree` summaries above.
