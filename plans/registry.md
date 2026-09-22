# Planning Registry

Updated: 2026-09-22
Current maintenance/CodeGG-quality baseline: `4a713ff82cec701534e285bbe3d330ae121f352c`
Current baseline audited for deployment work: `f595683b8ebdec0afb13363ec9e8ad7654f9824b` (`eggsearch` 0.3.8)
Current baseline for first-binary-release hardening: `34b36d1004121ba9891bac17b1033b150e8f1a3d` (`eggsearch` 0.3.8 on `main`)
Current baseline for eggfetch transport consolidation: `ac394031793cf5e37c49b790794e845ef0ab3650` (`eggsearch` 0.3.9 on `main`)
Current baseline for transport migration qualification closure: `eb014eb92ff06ad1653eb31518ae612d7de4dec1` (`eggsearch` 0.3.9 on `main`)
Current baseline for performance optimization: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`eggsearch` 0.3.9 on `main`)
Current baseline for performance timeout requalification: `0af540b8c4f7ec27678d83a74ba82aad45556f00` (`eggsearch` 0.3.9 on `main`)
Current baseline for eggfetch 0.2.0 adoption: `0c33d576a802b84ffdae5b8b52364a50646d111c` (`eggsearch` 0.3.9 on `main`)
Current baseline for Eggress 1.0.8 outbound routing integration: `dfa90e050c5434f3346902aeb4074901c58e90d1` (`eggsearch` 0.3.9 on `main`)
Previous search-workstream baseline: `e645a3fe42090fb7b7e1ce8639681fe69878f57b` (`eggsearch` 0.3.7)

## Completed workstream — Search capability expansion

| Workstream | Status | Depends on | Plan |
|---|---|---|---|
| Provider capability realization and Brave completion | complete | none | `phase-1-provider-request-contract-and-brave-realization.md` |
| Extractive evidence and fetch/cache controls | complete | phase 1 | `phase-2-extractive-evidence-and-fetch-control.md` |
| Firecrawl Developer Index | implemented | phases 1-2 | `phase-3-firecrawl-developer-index.md` |
| Exa semantic search provider | implemented | phases 1-2 | `phase-4-exa-semantic-search-provider.md` |
| Tavily search provider and closure pass | implemented | phases 1-2 | `phase-5-tavily-provider-and-closure.md` |

The governing rationale and cross-phase invariants for this workstream are in `roadmap.md`.

## Completed workstream — Binary distribution and deployment

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 6 | Release binaries and bootstrap installers | implemented | none | `phase-6-release-binaries-and-bootstrap-installers.md` |
| 7 | Binary-first self-update | implemented | phase 6 asset contract | `phase-7-binary-first-self-update.md` |
| 8 | Persistent Streamable HTTP MCP | implemented | none; coordinated with phase 6 release smoke | `phase-8-persistent-streamable-http-mcp.md` |
| 9 | Startup supervision, croncheck, and restart | implemented | phases 7-8 | `phase-9-startup-supervision-croncheck-and-restart.md` |
| 10 | Agent/IDE integration and deployment closure | implemented | phases 6, 8, 9 | `phase-10-agent-ide-integration-and-deployment-closure.md` |

The governing rationale, target matrix, installer/update contract, lifecycle split, and cross-phase invariants are in `deployment-roadmap.md`.

## Completed workstream — Maintenance and CodeGG retrieval quality

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 11 | Architecture and workflow consolidation | implemented | none | `phase-11-architecture-and-workflow-consolidation.md` |
| 12 | Provider probe and diagnostics closure | implemented | phase 11 preferred | `phase-12-provider-probe-and-diagnostics-closure.md` |
| 13 | Structured local code intelligence and repo-map enrichment | implemented | phase 11 | `phase-13-structured-local-code-intelligence-and-repo-map.md` |
| 14 | Retrieval ergonomics and focused batch evidence | implemented | phase 11; phase 13 preferred | `phase-14-retrieval-ergonomics-and-focused-batch-evidence.md` |
| 15 | Public API, docs, tests, and repository-hygiene closure | implemented | phases 11-14 | `phase-15-api-docs-tests-and-repository-hygiene-closure.md` |

The governing rationale, scope boundaries, cross-phase invariants, and stop conditions are in `maintenance-codegg-quality-roadmap.md`.

### Historical implementation order

```text
phase 11 -> phase 12
    |
    +-----> phase 13 -> phase 14 -> phase 15
```

Phase 12 may proceed in parallel once phase 11's execution seams are stable. Phase 15 is the closure pass.

### Workstream stop conditions

Do not mark this workstream complete until:

- the MCP and metasearch coordination layers are decomposed without replacing them with new monoliths;
- common repo/research/security workflow mechanics are shared while domain policy remains typed;
- the historical integration mega-suite is partitioned by behavioral contract;
- MCP provider probing is real, bounded, and uses the same core service as CLI diagnostics;
- provider capability documentation and descriptors agree;
- local search has a bounded structured symbol backend with regex fallback;
- `repo_map` exposes useful deterministic package/module/symbol/test/build structure;
- `batch_fetch` supports per-item focused evidence with an aggregate response budget;
- the accidental root `typescript` transcript and similar artifacts are removed and guarded against;
- the intended Rust public API boundary is explicit;
- CodeGG contracts and the routine verification gates pass on the exact closure candidate.

## Active corrective workstream — First binary-enabled release hardening

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 16 | First binary release hardening and cutover | implemented | implemented phases 6-10 | `phase-16-first-binary-release-hardening-and-cutover.md` |

Phase 16 addressed the binary-distribution implementation landing after the
`v0.3.8` tag/release. The first binary-enabled release is now published as
`v0.3.9`.

Do not backfill or move `v0.3.8`. The corrective path is to qualify the exact next release candidate before crates.io publication, then tag and release that same SHA as the first binary-enabled release.

### Phase 16 closure evidence

Closure evidence: qualified commit `0cbbeee79a34b7f6d2d226cef535096adc58b4c3`
was published as crate `0.3.9`, tagged as `v0.3.9`, and released at
<https://github.com/eggstack/eggsearch/releases/tag/v0.3.9>. Qualification run
`34653366561` and tagged release run `34655458760` passed the complete
seven-target matrix and exact 16-asset assembly. The published asset set and
all seven checksums were independently verified. Unix latest and pinned
installer plus updater smoke passed locally; native Windows latest and pinned
PowerShell installer plus `update --check` smoke passed in run `34660182644`.

The closure criteria were:

- the complete seven-target matrix can run in a non-publishing qualification mode before crates.io publication;
- qualification and tagged release share the same build, smoke, checksum, and assembly logic rather than parallel implementations;
- release preflight proves the exact checkout contains all required packaging inputs;
- workflow, installers, updater, docs, and assembly agree on one machine-checked target/asset contract;
- assembly validates exact asset-set equality;
- installer fallback/fail-closed behavior is covered by deterministic tests independent of a live GitHub Release;
- documentation does not advertise a dead latest-release installer URL before the first binary-enabled release exists;
- the exact SHA qualified before publication is the SHA eventually tagged;
- the new GitHub Release contains seven executables, seven checksums, `install.sh`, and `install.ps1`;
- external Unix and Windows bootstrap smoke proves the published installer downloads a release binary rather than silently taking the Cargo fallback path;
- `eggsearch update --check` resolves the same released version/asset contract.


## Active maintenance workstream — HTTP transport consolidation

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 17 | eggfetch 0.1.7 HTTP transport consolidation | implemented | phase 16 implemented; eggfetch-core 0.1.7 | `phase-17-eggfetch-0.1.7-http-transport-consolidation.md` |

Phase 17 migrated eggsearch-owned outbound HTTP from direct reqwest 0.12 to eggfetch-core 0.1.7 while preserving the rmcp-owned reqwest Streamable HTTP client boundary. The phase specifically adopts eggfetch's resolved-target connection reuse, corrected total-deadline body lifecycle, typed failures, and selective feature split without moving eggsearch SSRF/retry/truncation policy into the transport library.

### Phase 17 closure evidence

Implementation commit `5a739a3cc6cdd060911eeefad7b004661f4b5c94`
(+1945/-1448 across 76 files): Rust 1.89 declared in Cargo/CI/docs;
`eggfetch-core 0.1.7` with the bounded feature budget carries all
eggsearch-owned HTTP; direct reqwest 0.12 is removed (remaining normal-graph
reqwest 0.13.4 is rmcp-transitive by design, guarded); `FetchClient` owns one
shared client with per-hop pinned resolved snapshots and manual redirect
authorization; total deadlines cover body streaming; `OriginController`
remains the sole retry/circuit authority with typed failure evidence;
updater/provider redirects stay bounded with downgrade denial; `make check`
passes with zero test failures. HTML scrape engines request identity encoding
pending upstream `eggstack/eggfetch#24` for chunked compressed-response
decoding. Seven-target qualification is supplied by phase 18: run
`35427685324` (`mode=qualify`,
`ref=f9a661886376dd20c1539e20f990c43115ffdb90`) passed all seven targets and
exact 16-asset assembly on descendant candidate `f9a6618` containing this
implementation (see phase 18 implementation record). Qualification is
SHA-specific; re-qualify if the eventual release candidate differs.

Phase 17 migrates eggsearch-owned outbound HTTP from direct reqwest 0.12 to eggfetch-core 0.1.7 while preserving the rmcp-owned reqwest Streamable HTTP client boundary. The phase specifically adopts eggfetch's resolved-target connection reuse, corrected total-deadline body lifecycle, typed failures, and selective feature split without moving eggsearch SSRF/retry/truncation policy into the transport library.

### Phase 17 stop conditions

Do not mark phase 17 implemented until:

- Rust 1.89 is declared and exercised consistently because eggfetch 0.1.7 requires it;
- every eggsearch-owned HTTP path uses eggfetch and direct reqwest 0.12 is absent from production dependencies;
- the eggfetch feature graph is explicitly bounded and does not accidentally enable retry/Basic/proxy/H2/H3 capabilities not required by eggsearch;
- `web_fetch` still validates every redirect hop, pins the exact approved resolved-address snapshot, and preserves bounded truncation semantics;
- the fetch path uses one shared eggfetch client rather than constructing one transport client per destination;
- startup probes, provider requests, and updater requests use the corrected body-lifecycle total deadline with regression coverage;
- OriginController remains the sole application retry/circuit authority and consumes typed transport evidence where available;
- updater/provider redirect behavior remains bounded and secure, including updater downgrade denial;
- rmcp's reqwest-backed Streamable HTTP client remains an intentional, documented boundary rather than being replaced by an eggsearch-local adapter;
- before/after normal dependency graphs and representative linked release-binary sizes are recorded;
- `make check`, `make packaging-check`, `make release-check`, and the complete seven-target release qualification pass on the exact closure candidate.



## Active corrective workstream — Transport migration qualification closure

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 18 | Transport migration qualification and upstream compression closure | implemented | phase 17 implementation/closure | `phase-18-transport-migration-qualification-and-upstream-compression-closure.md` |

Phase 18 closed the phase 17 qualification gap without reopening transport architecture. Candidate `f9a661886376dd20c1539e20f990c43115ffdb90` passed local gates and qualification run `35427685324` (`mode=qualify`, `QUALIFIED_SHA=f9a661886376dd20c1539e20f990c43115ffdb90`, package `0.3.9`): all seven targets plus exact 16-file assembly with valid checksums and `0.3.9` version. Per-target sizes recorded as post-migration baseline. HTML scrape engines (`brave`, `duckduckgo`, `mojeek`, `searxng`, `startpage`, `yahoo`) retain `.decompress(false)` with static-guard and wire `Accept-Encoding` regression coverage; JSON APIs, fetch, updater, and probes retain automatic decompression. Upstream chunked gzip/Brotli defect reproducibly tracked at `eggstack/eggfetch#24` with deterministic loopback reproducer covering both codecs. No eggsearch decompression stack, direct reqwest, or unpublished pin. Qualification is SHA-specific; re-qualify a different eventual release candidate before publication.

### Phase 18 stop conditions

Do not mark phase 18 implemented until:

- a clean exact candidate containing the phase 17 implementation passes the normal local/release gates;
- `release-binaries.yml` runs with `mode=qualify` and `ref=<exact SHA>`;
- all seven target jobs and the final assembly job pass;
- the qualification artifact is inspected and contains exactly seven binaries, seven checksums, and two installers with valid checksums;
- the workflow run ID, `QUALIFIED_SHA`, artifact name, target results, and per-target sizes are recorded;
- affected HTML scrape engines retain deterministic regression coverage for the identity-encoding workaround;
- a deterministic upstream eggfetch reproducer/tracking record exists for the chunked compressed-response defect, or an upstream fixing commit plus regression test is identified;
- no eggsearch decompression compatibility layer, direct reqwest dependency, or unpublished eggfetch pin is introduced;
- phase 17 closure evidence is reconciled with the exact qualification run;
- the record explicitly states that qualification is SHA-specific and must be rerun before release if the eventual release candidate differs;
- phase 18's implementation record and registry status are updated together with exact evidence.


## Completed performance workstream — Hot-path optimization and footprint qualification

Governing rationale and cross-phase invariants: `performance-optimization-roadmap.md`.

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 19 | Performance baseline and local-search hot paths | implemented | phases 17-18 implemented | `phase-19-performance-baseline-and-local-search-hot-paths.md` |
| 20 | Fetch/cache sharing and timeout connection reuse | implemented | phase 19 benchmark conventions preferred | `phase-20-fetch-cache-and-connection-reuse.md` |
| 21 | MCP response shaping, focus projection, and discovery caching | implemented | phase 19 benchmark conventions preferred | `phase-21-mcp-response-shaping-and-discovery-caching.md` |
| 22 | Dependency-footprint qualification and performance closure | implemented | phases 19-21 | `phase-22-dependency-footprint-and-performance-closure.md` |
| 23 | Timeout override semantics and performance evidence requalification | implemented | phases 19-22 implementation/closure | `phase-23-timeout-semantics-and-performance-requalification.md` |

### Intended implementation order

~~~text
phase 19
   |
   +----> phase 20
   |
   +----> phase 21
              \
               -> phase 22 original closure
                          |
                          -> phase 23 corrective requalification
~~~

Phases 20 and 21 proceeded from the Phase 19 production-shaped benchmark conventions. Phase 22 was the original closure and dependency-footprint qualification pass; Phase 23 subsequently corrected the widened-timeout semantic defect, completed the missing benchmark evidence, and reran exact-candidate release qualification.

### Performance workstream closure evidence

Implementation candidate `5a8822ba538e89f9b8f441a328925fab78300754` contains
the original phases 19-22 changes. It passed `make check`,
`make packaging-check`, `make bench-check`, and the clean-tree release gate.
Rust 1.98.1 on `x86_64-apple-darwin` produced a default release binary of
18,744,624 bytes, compared with the 18,727,936-byte baseline. Tokio's direct
`full` feature was replaced with the qualified explicit set; rmcp's client,
child-process, and Streamable HTTP client features remain because integration
verification uses them.

Phase 23 closed the post-audit gaps on corrective candidate
`0af540b8c4f7ec27678d83a74ba82aad45556f00`: widened timeout overrides now
receive a widened client-scoped connect policy, the missing Phase 19-21
benchmark evidence was added, and release qualification run `35542118569`
passed all seven targets plus exact 16-file assembly. Current `main` is a
documentation-only descendant of that qualified production candidate.

### Performance workstream closure criteria

The workstream was closed only after:

- warm local search uses shared cached inventory snapshots rather than deep-cloning the complete `WorkspaceInventory`;
- candidate selection computes each candidate score once and preserves deterministic legacy tie ordering;
- equal/shorter timeout overrides preserve the shared eggfetch client/connection pool, while longer overrides build one widened client so client-scoped connect semantics remain correct;
- batch timeout setup is shared across item futures rather than rebuilt per item;
- internal derived-cache hits avoid one complete cached-document copy while retaining public compatibility and exact cache accounting;
- MCP projection/focus removes the audited JSON clone/deserialization round trips without changing compact/standard/diagnostic contracts;
- static advertised tool metadata/fingerprint construction is cached or measurement proves it negligible;
- direct Tokio feature activation is mechanically qualified and narrowed if safe;
- rmcp client transports used by `integrate --apply` verification are retained unless an equivalent supported path proves they can be narrowed without capability loss;
- targeted before/after performance evidence is recorded against comparable environments;
- normal correctness, packaging, and release gates pass on the exact closure candidate.

The workstream is explicitly optimization-with-equivalence. It must not change the ten-tool MCP surface, ranking weights, trust/SSRF semantics, cache policy, batch budget semantics, public Rust compatibility, browser/PDF availability, provider coverage, integration verification, or the Phase 18 compression workaround merely to improve benchmark or binary-size numbers.

## Corrective closure evidence — Phase 23

Phase 23 is implemented on corrective candidate
`0af540b8c4f7ec27678d83a74ba82aad45556f00`. It restores widened-timeout
connector semantics, adds the missing Phase 20/21 benchmark evidence, replaces
the Phase 19 unlike-workload timing comparison with an apples-to-apples
selector comparison, and passes seven-target non-publishing qualification
`35542118569` with exact 16-file assembly. The phase record retains the
historical Phase 19-22 measurements and explicitly reconciles their evidence
limits.

The qualified timeout policy is hybrid under eggfetch 0.1.7 resolved-route
semantics: equal/shorter overrides retain the shared client and request-level
deadline, while longer overrides build one widened client per top-level
fetch/batch so physical connect policy is widened as well. The performance
workstream is closed; any later production or dependency change that
invalidates the exact candidate must be separately qualified under the normal
release rules.

## Active corrective workstream — eggfetch 0.2.0 adoption and compression-workaround retirement

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 24 | eggfetch 0.2.0 adoption and compression-workaround retirement | implemented | phases 17-18 and phase 23 implemented; published eggfetch-core 0.2.0 | `phase-24-eggfetch-0.2.0-adoption-and-compression-workaround-retirement.md` |

Phase 24 consumes the published upstream correction for
`eggstack/eggfetch#24`. It bumps the direct transport dependency from
`eggfetch-core 0.1.7` to published `0.2.0`, removes the six temporary
HTML-engine `.decompress(false)` workarounds only after deterministic
downstream proof, restores normal gzip/Brotli negotiation, and requalifies the
resulting dependency/production-policy change. The pass must preserve the
Phase 17 ownership boundary and the Phase 23 timeout/client-reuse semantics.

The required downstream proof is not limited to upstream release notes:
eggsearch must exercise chunked gzip and Brotli through its own
production-equivalent `bytes_stream()` / `read_bounded_body()` path, retain
decoded-body limits and deadlines, keep the bounded eggfetch feature graph,
and run the full seven-target non-publishing release qualification on the exact
candidate before closure.

### Phase 24 closure evidence

Implemented on candidate `bac6f49fc046a93d7c094a8f96e1027631f390f1`:
crates.io-resolved `eggfetch-core 0.2.0` with the unchanged bounded feature
selection; six issue-#24 `.decompress(false)` call sites removed (Brave HTML,
DuckDuckGo, Mojeek, SearXNG, Startpage, Yahoo); deterministic chunked
gzip/Brotli + transfer-shape + decoded-limit + deadline regression coverage in
`provider_request_contract` (21/21); Phase 23 timeout semantics intact; local
correctness/clippy/docs/packaging/publish-dry-run gates pass; release binary
18,744,464 bytes (negligible delta); DuckDuckGo/Startpage live smoke
classified (H2 responses, no decompression errors); seven-target
qualification run `35692096012` passed all targets plus exact assembly; CI on
main passed. Full evidence in the Phase 24 implementation record.

### Phase 24 stop conditions

Do not mark Phase 24 implemented until:

- crates.io-resolved `eggfetch-core 0.2.0` is in `Cargo.toml`/`Cargo.lock`
  with the existing bounded feature selection;
- the Brave HTML, DuckDuckGo, Mojeek, SearXNG, Startpage, and Yahoo
  issue-#24 `.decompress(false)` call sites are removed;
- deterministic downstream integration tests prove chunked gzip and Brotli
  decode correctly through the same bounded-body path used in production;
- decoded-body size limits and compressed-response deadline behavior remain
  enforced;
- the Phase 23 equal/shorter shared-client and longer-timeout widened-client
  behavior remains intact;
- no local decompression stack, direct eggsearch reqwest dependency, or
  additional retry layer is introduced;
- local correctness/API/security/package gates pass;
- dependency/feature and representative binary-size deltas are recorded;
- DuckDuckGo/Startpage live smoke is attempted when reachable and any failure
  is classified;
- a clean immutable candidate passes all seven targets plus exact asset
  assembly in `release-binaries.yml` `mode=qualify`;
- current-state documentation no longer describes the identity-encoding
  workaround as active while Phase 17/18 historical evidence remains truthful;
- the Phase 24 implementation record and registry status are closed together
  with exact evidence.

## Active maintenance workstream — Eggress 1.0.8 optional outbound routing

| Phase | Workstream | Status | Depends on | Plan |
|---|---|---|---|---|
| 25 | Eggress 1.0.8 optional outbound proxy-chain integration | planned / ready for handoff | Phase 24 implemented; published eggfetch-core 0.2.0; published eggress-outbound/eggress-uri 1.0.8 | `phase-25-eggress-1.0.8-optional-outbound-proxy-chain-integration.md` |

Phase 25 adds Eggress only beneath eggfetch's custom-Dialer boundary. Eggfetch
remains the HTTP, pooling, destination-TLS, decompression, redirect-mechanics,
and deadline owner; Eggress is limited to listener-free physical TCP
proxy-chain establishment. The initial feature budget is ordinary HTTP/SOCKS
TCP routing only, with no eggress-embed, pproxy compatibility, SSH, QUIC/H3,
UDP, legacy crypto, insecure TLS, service listener, or direct-fallback behavior.

The primary acceptance gate is the existing resolved-address/SSRF contract.
Untrusted dynamic fetch targets may use Eggress only if the implementation can
prove that the physical route remains bound to the address snapshot authorized
by eggsearch while preserving the logical hostname for HTTP/TLS. If the
eggfetch 0.2.0 custom-Dialer seam cannot express that safely, those paths remain
on the existing pinned direct route and Eggress is constrained to explicitly
qualified fixed/operator-owned consumers. A process-global hostname-to-IP
side-channel is not an acceptable substitute.

The phase also requires an explicit dependency/packaging decision after
measurement: either one canonical default/release binary includes the feature,
or Eggress remains a documented source-build opt-in. Multiple binary SKUs are
not authorized.

## Deferred by design

### Search/research extensions

- recursive crawling or autonomous browser interaction;
- provider-generated answers, summaries, deep-research agents, or schema-generation layers;
- a new general-purpose `site_map` MCP tool unless a future evidence-based plan promotes it;
- Firecrawl Research Index passage/citation-graph operations unless separately planned;
- new general web providers that duplicate existing evidence classes;
- mandatory vector/embedding local indexing;
- mandatory LSP/rust-analyzer local-search dependency;
- full PDF layout/OCR work unless separately planned after the maintenance closure.

### Distribution/deployment extensions

- apt/RPM/Homebrew/Winget/Chocolatey/Scoop/MSI/PKG package pipelines;
- containers as the primary install mechanism;
- unattended/background auto-update scheduling;
- non-loopback/LAN/public MCP exposure and authentication;
- multiple feature-specific binary SKUs;
- board-specific CPU-tuned or branded Raspberry Pi/Le Potato assets;
- MCPB as a required distribution mechanism until architecture-selection behavior is sufficient.

## Closure rule

A phase is `implemented` only when its own acceptance criteria have been exercised against the exact candidate and its status/registry entry are updated in the same closure commit. If implementation discovers a correctness/security/portability blocker, mark the phase `blocked` and write a scoped corrective plan rather than weakening acceptance criteria silently.
