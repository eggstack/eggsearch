# Repository Security, Supply-Chain, and Maintenance Hardening Roadmap

Status: active; M001-M005 closed, M006 externally blocked

Planning baseline: `c49c600b76e690bb1bc52f641554cca6bf79f36f`

Long-term references:

- `plans/000-long-term-specification.md#4` — trust and safety bounds.
- `plans/000-long-term-specification.md#6` — distribution and operations.
- `plans/000-long-term-specification.md#9` — verification gates.
- `plans/002-long-term-roadmap.md`.

Related ADRs:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`.
- `plans/adrs/ADR-0003-outcome-b-optional-outbound-routing.md`.

## 1. Purpose and ownership boundary

Close current dependency, CI/release supply-chain, process-execution, unsafe-code,
and security-sensitive maintenance gaps without changing eggsearch's ten-tool MCP
surface, CLI capability, provider coverage, fetch semantics, or CodeGG contract.

This workstream owns the repository-level policy and implementation seams around:

- Cargo dependency security and dependency-policy enforcement;
- GitHub Actions and release provenance;
- the remaining transitive `reqwest` graph and eggfetch transport ownership;
- external process execution, output bounds, timeout/kill behavior, and the small
  Unix unsafe boundary required for process groups/sessions;
- decomposition of security-sensitive high-density modules where change locality
  is itself an auditability risk.

It does not own search ranking, provider semantics, dependency-evidence meaning,
MCP request/response schemas, or new network capabilities.

## 2. Work classification

### Invariants

- The ten MCP tools and their wire contracts remain unchanged.
- Outbound HTTP owned by eggsearch uses `eggfetch-core`; no new eggsearch-local
  reqwest client/facade is permitted.
- Dynamic web fetch SSRF, redirect, resolved-address pinning, body limits, timeout,
  sanitation, and cache semantics do not weaken.
- Existing optional browser, PDF, and egress capabilities remain available.
- External process execution never introduces shell interpolation.
- Unsafe code is confined to a documented platform helper where no safe equivalent
  provides the required session/process-group behavior.
- Historical closure evidence for the earlier transport, maintenance, distribution,
  and tool-surface workstreams remains intact.

### Infrastructure

- Automated dependency advisory/license/source policy.
- Immutable/pinned CI action references and release provenance attestations.
- Shared bounded process-execution primitive.
- Static guards preventing reintroduction of retired dependency/process patterns.

### Polish / maintenance

- Security-sensitive module decomposition with stable module/API seams.
- Documentation and ownership maps reconciled with the landed implementation.

## 3. Non-goals

- New providers or MCP tools.
- Non-loopback MCP exposure or authentication redesign.
- Replacing eggfetch with another HTTP stack.
- Removing browser/PDF/egress functionality to make dependency graphs smaller.
- A permanent eggsearch-private chromiumoxide fork without a separate explicit
  architecture decision.
- Rewriting rmcp or chromiumoxide protocol implementations locally.
- Requiring a second release SKU.
- Turning dependency warnings into blanket deny rules without classifying current
  unavoidable transitive dependencies.

## 4. Current state at the planning baseline

### Dependency security

`Cargo.lock` resolves:

- `quick-xml 0.38.4`. RUSTSEC-2026-0194 affects the default checked
  `BytesStart::attributes()` path and is fixed in `>=0.41.0`. Eggsearch's
  `.csproj` parser iterates that path in
  `src/meta/dependency_parse/dotnet.rs`, so the advisory is reachable on
  attacker-controlled dependency evidence.
- `rustls 0.23.43`. RUSTSEC-2026-0285 affects
  `>=0.23.13,<0.23.45`. The production graph reaches rustls through
  `eggfetch-core` / `hyper-rustls`.
- `ttf-parser 0.25.1` through optional `lopdf`. RUSTSEC-2026-0192 marks the
  crate unmaintained and provides no patched ttf-parser release. This is a
  maintenance finding, not justification to regress PDF capability.

The lock already contains patched versions for other recently relevant notices
reviewed during planning (for example `h2 0.4.19`, `anyhow 1.0.104`,
`lru 0.18.2`).

Routine CI runs fmt, clippy, feature checks, tests, repository hygiene, and
packaging checks, but there is no RustSec/license/source policy gate.

### CI and release supply chain

Workflows use movable action tags such as `actions/checkout@v4`,
`actions/upload-artifact@v4`, `dtolnay/rust-toolchain@stable`,
`mlugg/setup-zig@v2`, and `docker/setup-qemu-action@v3`.
The release assembly job correctly narrows write permission to the job that
publishes assets, but release artifacts have no GitHub artifact attestation.
The existing release workflow already follows the useful draft -> assemble ->
manual publish shape required for a clean immutable-release transition.

### Remaining reqwest graph

Eggsearch has no direct reqwest dependency. `Cargo.lock` contains one reqwest
package (`0.13.4`) and two direct reverse dependencies:

1. `rmcp 3.2.0`, because Cargo enables
   `transport-streamable-http-client-reqwest`;
2. `chromiumoxide 0.9.1`, which declares reqwest unconditionally even when
   eggsearch selects `default-features = false`.

`src/integrations/common.rs::verify_http()` already uses eggfetch for the
health request, then switches to rmcp's reqwest-backed
`StreamableHttpClientTransport` for MCP initialization and `tools/list`.
rmcp 3.2.0 also offers a transport-agnostic Streamable HTTP client seam, but a
custom backend must correctly implement SSE parsing, cancellation, and raw
event-size bounds. For eggsearch's local registration verification, retaining a
general streaming HTTP client is unnecessary: the repository already exercises
the HTTP MCP protocol directly with eggfetch in `tests/mcp_http.rs`.

Chromiumoxide is different. Current upstream source uses reqwest only for the
HTTP(S) `Browser::connect()` discovery probe that resolves
`/json/version`; eggsearch's browser lifecycle uses `Browser::launch()`.
The dependency remains in the graph solely because chromiumoxide does not make
that HTTP-discovery dependency optional.

### Process and unsafe boundary

`src/meta/local_inventory_cache.rs` has a mature bounded command runner with
concurrent stdout/stderr draining, output caps, deadlines, and process-group
termination. `src/startup.rs` still has separate `.output()` /
`.status()` helpers, including unbounded captured output for `crontab -l`
and generic command probes.

Both startup and local git execution contain their own Unix
`CommandExt::pre_exec` + `libc::setsid()` unsafe blocks. The behavior is
legitimate, but duplicated unsafe process-session setup increases maintenance
and review risk.

## 5. Research basis

Primary current references used to form this roadmap:

- RUSTSEC-2026-0194:
  https://rustsec.org/advisories/RUSTSEC-2026-0194.html
- RUSTSEC-2026-0285:
  https://rustsec.org/advisories/RUSTSEC-2026-0285.html
- RUSTSEC-2026-0192:
  https://rustsec.org/advisories/RUSTSEC-2026-0192
- GitHub Actions secure-use guidance (full-SHA action pinning):
  https://docs.github.com/en/actions/reference/security/secure-use
- GitHub artifact attestations:
  https://docs.github.com/en/actions/concepts/security/artifact-attestations
- GitHub immutable releases:
  https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
- rmcp 3.2.0 feature inventory:
  https://docs.rs/crate/rmcp/3.2.0
- rmcp generic Streamable HTTP client seam:
  https://docs.rs/rmcp/latest/rmcp/transport/streamable_http_client/trait.StreamableHttpClient.html
- chromiumoxide upstream:
  https://github.com/mattsse/chromiumoxide

## 6. Target architecture

The intended end state is:

```text
Cargo manifest/lock
    |
    +--> cargo-deny / RustSec / source-license policy ------> CI fail closed
    |
GitHub Actions
    |
    +--> full-SHA action references
    +--> least-privilege publish job
    +--> provenance attestations
    +--> immutable future releases
    |
eggsearch HTTP
    |
    +--> eggfetch-core --------------------> providers/fetch/update/probes
    +--> rmcp server transport ------------> server-side MCP only
    +--> rmcp child-process client --------> stdio integration verification
    `--> no rmcp reqwest HTTP client
             |
             `--> chromiumoxide is the only temporary reqwest residue
                       |
                       `--> removed after upstream optional HTTP discovery
    |
process execution
    |
    +--> one bounded runner
    +--> one process-session/group helper
    +--> one narrow documented unsafe location
    `--> static guards prevent bypass
```

## 7. Dependency graph

```text
M001 dependency remediation + policy gate ---------+
                                                    |
M002 CI/release supply-chain provenance ------------+---- can proceed in parallel
                                                    |
M003 rmcp HTTP verification on eggfetch ------------+
                                                    |
M004 process execution + unsafe boundary -----------+----> M005 forge safety decomposition
                                                          (hard dependency on M004 only)

M003 closed + chromiumoxide upstream optionalization/release
        |
        `-----------------------------------------------> M006 zero-reqwest closure
                                                           (external hard blocker)
```

M001-M004 have no hard dependency on one another. They should still land as
separate commits/candidates so a dependency update, workflow hardening,
transport-feature change, and process refactor remain independently reviewable.

## 8. Milestones

Current milestone state after implementation: M001, M003, M004, and M005 are
closed; M002 is conditionally closed pending attestation and immutable-release
evidence from the next tagged release. M005's M004 dependency is cleared.
M006's M003 dependency is cleared, leaving only its external upstream
chromiumoxide release blocker. Per-milestone evidence is in
`plans/closure/repository-hardening/`.

### M001 — Dependency remediation and enforceable dependency policy

Primary class: infrastructure + security invariant.

Patch the reachable quick-xml and rustls advisories, establish a checked-in
dependency policy, add advisory/license/source checks to CI, and make
exception/expiry handling explicit.

Implementation plan:
`plans/implementation/repository-hardening/001-dependency-security-and-policy-gate.md`.

### M002 — CI and release supply-chain provenance

Primary class: infrastructure.

Pin every external GitHub Action to an immutable full commit SHA, enforce that
shape mechanically, add release artifact provenance attestations without
changing the 16 release-asset contract, and operationalize immutable future
releases.

Implementation plan:
`plans/implementation/repository-hardening/002-ci-release-supply-chain-provenance.md`.

### M003 — Retire rmcp's reqwest HTTP client in favor of eggfetch

Primary class: infrastructure + maintenance.

Move `integrate --apply --transport http` verification onto eggfetch-backed
bounded MCP requests and remove the rmcp reqwest Streamable HTTP feature while
retaining rmcp's client/child-process feature for stdio verification.

Implementation plan:
`plans/implementation/repository-hardening/003-rmcp-http-verification-on-eggfetch.md`.

### M004 — Bounded process execution and unsafe-boundary consolidation

Primary class: infrastructure + security invariant.

Extract the existing robust git runner mechanics into a shared process owner,
migrate startup/process probes to bounded deadlines/output, and consolidate
Unix session/process-group setup into one narrowly allowed unsafe seam.

Implementation plan:
`plans/implementation/repository-hardening/004-process-and-unsafe-boundary-hardening.md`.

### M005 — Forge safety ownership decomposition

Primary class: maintenance.

After M004 reduces cross-cutting process ownership, decompose
`meta/forge_adapter` so URL/address/SSRF/read-budget policy has one small
auditable owner and host-specific request/tree mechanics cannot silently fork
that policy.

Implementation plan:
`plans/implementation/repository-hardening/005-forge-safety-maintenance-decomposition.md`.

### M006 — Full zero-reqwest dependency-graph closure

Primary class: maintenance + dependency footprint.

Blocked until chromiumoxide exposes an upstream release in which the HTTP
DevTools discovery dependency is optional (or an equivalently clean upstream
client-injection seam exists). Adopt that release with HTTP discovery disabled
for eggsearch's launch-only browser use, then prove `reqwest` is absent from
the all-features graph.

Implementation plan:
`plans/implementation/repository-hardening/006-chromiumoxide-zero-reqwest-closure.md`.

## 9. Cross-cutting requirements

### Compatibility

No milestone may change the ten tool names, request/response schemas, provider
IDs, default feature behavior, release target set, installer contract, browser
availability, PDF availability, egress behavior, CodeGG integration contract,
or loopback-only HTTP serving policy.

### Security

Security checks fail closed for known vulnerabilities. Policy waivers must be
specific, justified, time-bounded, and visible. No milestone may convert an
SSRF, redirect, output-size, timeout, or process-boundary failure into silent
success.

### Failure and recovery

Dependency-policy service/network failures must not be confused with a clean
audit. Release attestation failures block publication. Process timeouts must
terminate owned process groups and report a typed/bounded diagnostic.
Transport verification failure must leave client configuration reporting the
same failure semantics it has today.

### Performance/resources

Dependency and workflow guards should avoid adding a second full compilation to
ordinary CI where metadata/advisory checks suffice. Process output and MCP
verification responses remain bounded.

### Documentation

`architecture/security.md`, `architecture/maintenance.md`,
`architecture/fetch.md`, release docs, and testing inventories must move with
the milestone that changes their factual contracts.

## 10. Verification strategy

Every implementation milestone runs `make check` on its exact candidate plus
its focused security/transport/process suite. M002 additionally validates all
workflow files mechanically. M003/M006 record `cargo tree` evidence for
reqwest. M004 records process timeout/output-cap/process-group tests. M005
records forge SSRF/redirect/read-budget parity.

Release-facing changes require the repository's qualification workflow before
a publication candidate is accepted. Since this work does not publish a new
version, M002's code and repository setting are implemented while its first
release's generated/verified attestation and immutable behavior remain an
operational closure condition.

## 11. Risks and deferred work

- `ttf-parser` currently has no patched release; do not remove PDF support to
  produce a cosmetically clean advisory report. Track the transitive owner and
  use an explicit waiver if policy tooling requires one.
- Artifact attestations establish provenance, not artifact safety. Checksums
  remain part of the release contract.
- The self-updater currently verifies same-release-domain checksums before
  executing a candidate. Consuming Sigstore/GitHub attestations inside the
  updater is intentionally deferred until an attested immutable release exists
  and an in-process trust-root/verification design is separately accepted.
- Chromiumoxide must not be permanently forked merely to remove one dead-for-
  eggsearch HTTP dependency. If upstream declines an optional transport seam,
  revisit with a dedicated ADR instead of hiding a patch in Cargo configuration.
- Broad splits of `core/config.rs` and `core/security.rs` remain later
  maintenance candidates; this roadmap only decomposes the forge boundary where
  security policy and host mechanics are currently co-located.

## 12. Completion definition

The workstream is complete when:

- current known reachable dependency vulnerabilities are patched and CI enforces
  an explicit dependency policy;
- all external Actions used by eggsearch are immutable-SHA pinned and guarded;
- release binaries/installers have provenance attestations and future release
  immutability is enabled/verified;
- rmcp no longer brings reqwest into the default graph;
- production process capture/probes use bounded execution, and unsafe process
  session setup has exactly one documented owner;
- forge safety policy is decomposed without behavioral drift;
- chromiumoxide upstream support is adopted and `cargo tree --all-features -i
  reqwest` shows no reqwest package (M006 remains externally blocked);
- all milestones have SHA-specific closure records and no unresolved High/Medium
  finding is hidden by a waiver.
