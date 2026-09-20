# Performance Optimization and Footprint Roadmap

Status: planned
Baseline audited: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`, 2026-09-19)
Primary downstream consumer: `dbowm91/codegg`
Depends on: phases 17-18 implemented

## Objective

Reduce CPU work, allocation pressure, memory copying, connection churn, and avoidable binary/dependency footprint in eggsearch without changing the stable MCP surface, public response contracts, provider capability, trust/safety semantics, deterministic ordering, or supported deployment/integration behavior.

This is a hot-path consolidation workstream, not a redesign. The audited repository already has bounded provider fan-out, byte-bounded caches, shared provider HTTP clients, a mature Criterion suite, deterministic response projection, and the phase-17 eggfetch transport consolidation. The remaining opportunities are concentrated in data movement and repeated work inside already-correct pipelines.

The intended outcome is lower warm-path latency and allocation volume for CodeGG-style repeated repository/search/fetch workloads while keeping behavior observably equivalent.

## Current evidence

The audit identified four concrete optimization classes.

### Local workspace search

`LocalWorkspaceBackend` stores its inventory as `Arc<RwLock<Option<Arc<WorkspaceInventory>>>>`, but the warm search path currently dereferences the Arc and clones the complete `WorkspaceInventory`. That recursively copies root inventories, file-entry strings, and paths before each search.

Candidate selection also sorts all filtered candidates while calling `score_inventory_entry` from the comparator. The score function lowercases paths and filenames, so the current sort can perform allocation-bearing scoring O(N log N) times even though only approximately `max_results * 2` candidates survive.

The existing inventory Criterion cases exercise one score evaluation per entry. They do not benchmark the actual full candidate-selection path and therefore do not expose comparator recomputation.

### Fetch and cache hot paths

`FetchClient::with_timeout_ms` currently constructs a new `eggfetch_core::Client`. Eggfetch clients are cloneable shared handles around an inner client/connection pool, and fetch requests already apply request-level timeout overrides. Rebuilding the client for a timeout-only override therefore discards reusable transport state.

`batch_fetch` amplifies this when a timeout override is present because each web item can create a separate timeout-adjusted client.

The raw fetch cache already stores body bytes behind `Arc<[u8]>`, but the derived document cache returns a cloned owned `DerivedDocumentCacheEntry`. Cache hits can therefore deep-copy extracted text, links, and structured documents while the cache mutex is held, followed by additional cloning while constructing the response.

### MCP response shaping

Compact/standard projection operates on owned `serde_json::Value` trees but frequently clones subtrees before immediately removing the originals. Batch focus injection also clones an already-serialized `document` JSON value and deserializes it back into `FetchDocument` before computing focus.

The MCP tool contract is static for a running binary, but `tools/list` currently reapplies contract metadata, recreates output schemas, sorts tools, and recomputes the contract fingerprint on demand.

### Build/dependency footprint

The direct Tokio dependency currently uses `features = ["full"]`. A narrower explicit feature set may reduce build graph and linked footprint, but it must be derived from the all-feature/default/no-default build matrix rather than guessed.

The rmcp client/child-process/Streamable-HTTP-client features are not currently dead: `integrations/common.rs` uses them to verify both stdio and HTTP integrations after `integrate --apply`. They must not be removed merely because the primary runtime role is an MCP server. Any future reduction there would require preserving the same verification capability through a supported path and is not assumed by this workstream.

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

Phases 20 and 21 may proceed in parallel after Phase 19 establishes the performance-evidence conventions. Phase 22 is the closure/footprint pass.

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

## Workstream stop conditions

Do not mark this workstream complete until:

- warm local search no longer deep-clones the complete cached workspace inventory;
- local candidate scoring is computed O(N), not repeatedly from a full-sort comparator, while preserving deterministic ordering;
- timeout overrides no longer rebuild an eggfetch connection pool solely to change request limits;
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
