# Performance Optimization and Footprint Roadmap

Status: implemented (phases 19-23 closed)
Baseline audited: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`, 2026-09-19)
Primary downstream consumer: `dbowm91/codegg`
Depends on: phases 17-18 implemented

## Objective

Reduce CPU work, allocation pressure, memory copying, connection churn, and avoidable binary/dependency footprint in eggsearch without changing the stable MCP surface, public response contracts, provider capability, trust/safety semantics, deterministic ordering, or supported deployment/integration behavior.

This is a hot-path consolidation workstream, not a redesign. The audited repository already has bounded provider fan-out, byte-bounded caches, shared provider HTTP clients, a mature Criterion suite, deterministic response projection, and the phase-17 eggfetch transport consolidation. The remaining opportunities are concentrated in data movement and repeated work inside already-correct pipelines.

The intended outcome is lower warm-path latency and allocation volume for CodeGG-style repeated repository/search/fetch workloads while keeping behavior observably equivalent.

## Audit baseline and implemented outcome

The original audit identified four concrete optimization classes. Phases 19-23
have now addressed and qualified them; the descriptions below distinguish the
audited baseline from the current implementation.

### Local workspace search

At the audit baseline, warm local search dereferenced the cached
`Arc<WorkspaceInventory>` and cloned the complete inventory, and candidate
selection repeatedly recomputed allocation-bearing scores inside a full-sort
comparator.

The current implementation keeps shared inventory snapshots behind `Arc`,
computes candidate scores once, and uses bounded selection before deterministic
final ordering. Phase 23 added an apples-to-apples legacy full-sort versus
optimized-selector benchmark on identical 1,000- and 4,096-entry workloads.

### Fetch and cache hot paths

At the audit baseline, timeout adjustment rebuilt an `eggfetch_core::Client`
and `batch_fetch` could amplify that cost per web item. Derived-cache hits
also returned cloned owned entries, deep-copying extracted content on the
internal hot path.

The current implementation stores derived entries behind `Arc` internally
and prepares one adjusted fetch client per top-level batch. Timeout handling is
hybrid under eggfetch 0.1.7 resolved-route semantics: equal/shorter overrides
reuse the shared transport, while longer overrides build one client with the
widened client-scoped connect timeout. Phase 23 added timeout-adjustment,
derived-cache-hit, and batch-setup characterization.

### MCP response shaping

At the audit baseline, compact/standard projection cloned owned JSON subtrees
before removal, batch focus could serialize and deserialize a document that was
already available in typed form, and repeated tool discovery rebuilt static
contract metadata/fingerprint work.

The current implementation removes the audited avoidable projection clones,
passes typed web documents into batch focus where available, and caches the
advertised tool contract and fingerprint on the server. Phase 23 added repeated
`tool_definitions()` and `tool_fingerprint()` characterization. The repo
focus path retains its JSON fallback where no typed document is available.

### Build/dependency footprint

At the audit baseline, the direct Tokio dependency enabled
`features = ["full"]`. Phase 22 replaced that with the mechanically qualified
explicit feature set while retaining rmcp client/child-process/Streamable HTTP
client features required by `integrate --apply` verification.

The change is dependency/feature hygiene rather than a binary-size win: the
recorded default x86_64-apple-darwin release binary increased by 16,688 bytes
on the measured candidate. Phase 23 subsequently reran the exact-candidate
seven-target non-publishing qualification after all production/Cargo changes.

## Workstream principles

1. Preserve all ten stable MCP tools and their request/response compatibility.
2. Preserve public Rust APIs unless a change is strictly additive. Existing public owned-return APIs may gain internal shared fast paths, but must not be replaced with incompatible signatures.
3. Preserve ranking semantics, deterministic tie ordering, stable identity, cache policy, provider routing, timeout semantics, and warning/retrieval accounting.
4. Preserve SSRF controls, per-hop resolved-address pinning, bounded reads, sanitization, trust markers, and the Phase 18 identity-encoding workaround for affected HTML providers.
5. Prefer removal of copies/recomputation over algorithmic or dependency complexity.
6. Add performance evidence before or with the optimization. A microbenchmark that does not model the production path is not sufficient evidence.
7. Do not make CI timing-sensitive. Criterion results are characterization/evidence; deterministic correctness tests remain the gating contract.
8. Do not add a new cache framework, allocator, async runtime, persistent index, or transport abstraction for this campaign.
9. Treat binary-size reduction as secondary to capability and maintainability. No optimization may disable browser/PDF/integration behavior merely to shrink the binary.

## Phases

| Phase | Workstream | Depends on |
|---|---|---|
| 19 | Performance baseline and local-search hot paths | phases 17-18 implemented |
| 20 | Fetch/cache sharing and timeout connection reuse | phase 19 benchmark conventions preferred |
| 21 | MCP response shaping, focus projection, and discovery caching | phase 19 benchmark conventions preferred |
| 22 | Dependency-footprint qualification and performance closure | phases 19-21 |
| 23 | Timeout override semantics and performance evidence requalification | phases 19-22 implementation/closure |

## Intended implementation order

~~~text
phase 19
   |
   +----> phase 20
   |
   +----> phase 21
              \
               -> phase 22 closure
~~~

Phases 20 and 21 proceeded from the Phase 19 performance-evidence conventions. Phase 22 was the original closure/footprint pass. The post-closure audit identified a timeout-semantic regression plus benchmark/release-qualification evidence gaps; Phase 23 subsequently corrected and requalified those items, completing the workstream.

## Cross-phase invariants

### MCP and CodeGG contract

The stable tool names remain:

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`.

No response-detail mode may silently gain or lose fields except where an existing contract already permits optional omission. Compact/standard/diagnostic projection behavior must remain contract-tested.

### Local-search equivalence

Optimization may change how candidates are represented and selected internally, but the same inventory/config/query must preserve deterministic result ordering and tie behavior. Inventory freshness/rebuild semantics and safe-open validation remain authoritative.

### Fetch/cache equivalence

Timeout-only overrides must retain the exact request timeout, DNS-validation timeout derivation, redirect authorization, body cap, sanitization, cache policy, and error classification currently exposed. Connection-pool reuse is an implementation improvement, not a semantic change.

Cache entries must remain scoped and invalidated exactly as today. Sharing cached values internally must not allow mutation of a value still resident in the cache.

### Phase 18 transport boundary

Do not remove `.decompress(false)` from affected HTML scraping engines until the upstream eggfetch compression defect is resolved and separately qualified. Do not reintroduce reqwest or create an eggsearch decompression compatibility layer.

### Evidence

Each phase should record:

- exact baseline SHA;
- correctness gates executed;
- benchmark command and relevant benchmark names;
- before/after medians or distributions when the environment is stable enough for comparison;
- release-binary/dependency measurements when the phase changes manifest features;
- any optimization considered but rejected because the evidence did not justify complexity.

## Workstream closure evidence

Phases 19–22 were implemented on candidate
`5a8822ba538e89f9b8f441a328925fab78300754` and documented closed at
`a09a35019d4e6ba75a5787b6552fe4e6161ae1c9`. A post-closure audit found that
the runtime improvements are mostly sound but the closure is not final:
widened `FetchClient` timeout overrides can retain the shorter client-scoped
resolved-route connect timeout under eggfetch 0.1.7 semantics; registered
Phase 20/21 benchmark evidence is incomplete; the Phase 19 timing pairs are
not apples-to-apples; and the seven-target release qualification predates the
production/Cargo changes.

Phase 23 closed those corrective items on candidate
`0af540b8c4f7ec27678d83a74ba82aad45556f00`. It records corrected timeout
semantics, comparable benchmark evidence, local gates, and fresh exact-candidate
seven-target non-publishing qualification `35542118569` with exact 16-file
assembly. The implementation candidate remains the SHA-specific release
qualification reference; later documentation-only closure commits do not
change its production behavior.

## Workstream stop conditions

Do not mark this workstream complete until:

- warm local search no longer deep-clones the complete cached workspace inventory;
- local candidate scoring is computed O(N), not repeatedly from a full-sort comparator, while preserving deterministic ordering;
- equal/shorter timeout overrides do not rebuild an eggfetch connection pool solely to change request limits, while longer overrides intentionally build one widened client for connector semantics;
- batch fetch does not construct one timeout-adjusted transport client per item;
- derived-cache hot paths avoid one full deep copy of cached extracted documents without changing public compatibility;
- MCP projection/focus paths no longer perform avoidable JSON clone/deserialization round trips identified in the audit;
- static tool contract/schema/fingerprint work is cached or otherwise proven negligible;
- the direct Tokio feature set has been mechanically qualified and narrowed if doing so preserves all supported build/features;
- rmcp integration-verification features are retained unless an equivalent supported implementation proves they can be removed without loss;
- targeted benchmarks cover the production-shaped hot paths rather than only isolated helper functions;
- `make check`, packaging checks, and the relevant release/build matrix pass on the exact closure candidate.

## Explicitly deferred

The following are not justified by the current evidence and should not be pulled into this campaign:

- replacing Tokio with another async runtime;
- introducing a persistent database/index for local search;
- embedding lowercase copies of every path unless measurement proves the memory/CPU tradeoff worthwhile;
- continuous replenishment of `batch_fetch` if it changes deterministic aggregate-budget semantics;
- singleflight/request coalescing without evidence of duplicate concurrent fetch pressure;
- replacing rmcp's client transports with an eggsearch-owned MCP client;
- changing release profile semantics such as panic behavior or optimizing for size at the expense of request latency;
- removing Phase 18 compression workarounds before upstream qualification.
