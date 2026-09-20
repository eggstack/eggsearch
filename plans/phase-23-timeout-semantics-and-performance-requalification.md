# Phase 23 — Timeout Override Semantics and Performance Evidence Requalification

Status: planned
Depends on: phases 19-22 implementation candidate `5a8822ba538e89f9b8f441a328925fab78300754`; documentation closure `a09a35019d4e6ba75a5787b6552fe4e6161ae1c9`
Baseline for corrective planning: `a09a35019d4e6ba75a5787b6552fe4e6161ae1c9` (`main`)
Governing roadmap: `performance-optimization-roadmap.md`
Related transport baseline: `eggfetch-core 0.1.7`

## Why this corrective phase exists

The phases 19-22 performance campaign landed substantial valid improvements, but a post-closure audit found one semantic defect and two evidence/qualification gaps that make the closure record too strong.

This phase is intentionally narrow. It does not reopen the local-search, cache-sharing, projection, or Tokio-feature architecture that already landed correctly.

### Correctness defect — widened fetch timeout overrides no longer fully widen connect policy

Phase 20 changed `FetchClient::with_timeout_ms` from rebuilding an
`eggfetch_core::Client` to cloning the existing shared client and changing
only eggsearch's `FetchLimits.timeout_ms`.

Each request still applies:

~~~rust
builder.timeout(client_timeout(self.limits.timeout_ms))
~~~

and eggfetch merges those request-level timeout fields with the client
defaults. That is sufficient for request-scoped pool/read/write/total
deadlines.

However, eggfetch 0.1.7's advanced/resolved routing path treats the physical
connect timeout differently. The reusable Hyper connector is built from the
client-level `timeout.connect`, and eggfetch's own route-cache documentation
describes connect timeout as connection-affecting client-scoped policy.

eggsearch's hardened fetch path deliberately uses resolved-address routing
after DNS/SSRF validation, so this distinction applies to the production
`web_fetch` path.

The consequence is asymmetric:

- an equal or shorter override can safely reuse the base client because the
  stricter request total deadline bounds the attempt before the looser base
  connector timeout;
- a longer override cannot fully widen connect establishment while reusing a
  connector built with the shorter base connect timeout.

For example, a base fetch timeout of 8 seconds plus
`web_fetch(timeout_ms=30000)` may still have new resolved-route connection
establishment bounded by the original approximately 8-second client connect
policy. Before Phase 20, rebuilding the client with the override widened that
policy as well.

This violates the performance roadmap's explicit requirement that timeout
semantics remain unchanged.

### Evidence gap — benchmark coverage does not satisfy the registered plans

The implementation added useful Criterion cases for local candidate selection,
warm Arc acquisition, and MCP projection. It did not add all of the benchmark
coverage required by phases 20-21:

- no derived-cache shared-hit benchmark at realistic document sizes;
- no timeout-adjusted `FetchClient` construction/reuse benchmark;
- no batch timeout-client setup benchmark;
- no repeated tool-contract/`tools/list`/fingerprint access benchmark.

Phase 19's recorded pre/post numbers also compare different operations: the
old one-score-per-entry inventory microbenchmark versus the new
production-shaped candidate selector. Those values are useful
characterization, but not an apples-to-apples before/after performance claim.

### Qualification gap — the seven-target release matrix is stale

The last recorded non-publishing seven-target release qualification is Phase
18 run `35427685324` against candidate
`f9a661886376dd20c1539e20f990c43115ffdb90`.

Phases 19-22 subsequently changed production `src/`, `Cargo.toml`, and
`Cargo.lock`. Phase 18's own exact-candidate rule identifies all three as
release-qualification-invalidating changes.

Current-head CI passing is valuable but is not a substitute for the
seven-target `release-binaries.yml` qualification matrix and exact 16-asset
assembly contract.

## Objective

Restore exact timeout-override semantics without discarding the valid
connection-reuse improvements, fill the missing performance evidence, and
requalify the resulting exact candidate across the release matrix.

The desired end state is:

- equal/shorter timeout overrides reuse the shared eggfetch client and its
  route/connection pools;
- longer timeout overrides use a client whose physical connect policy is
  actually widened to the requested timeout;
- `batch_fetch` still constructs at most one timeout-adjusted client per
  batch rather than one per item;
- normal and conditional fetch paths apply the same effective timeout
  semantics;
- phase 20 and phase 21 benchmark obligations have production-shaped coverage;
- local-search performance evidence is made apples-to-apples rather than
  comparing unlike operations;
- the exact corrective candidate passes the complete seven-target
  non-publishing qualification and exact asset assembly.

## Ownership boundary

Preserve the Phase 17/18 transport ownership split.

~~~text
eggsearch
  owns the user-facing timeout override contract, DNS/SSRF validation,
  resolved-address selection, redirect authorization, retry/origin policy,
  batch orchestration, cache policy, and evidence/qualification records

eggfetch
  owns transport connection pooling, physical connector timeout behavior,
  request timeout merging, transfer decoding, and connection lifecycle

rmcp
  continues to own its MCP client/server transport implementation and
  transitive reqwest boundary
~~~

Do not patch eggfetch internals inside eggsearch.

Do not consume an unpublished eggfetch commit to close this phase.

If a published eggfetch release is available during implementation and
changes resolved-route connect timeout semantics, treat adoption as a
separate dependency decision: verify the published behavior first, update the
plan/registry scope explicitly, and do not silently replace the corrective
strategy below.

## Work item 1 — Lock timeout override semantics with focused tests

Before changing `with_timeout_ms`, add deterministic tests that distinguish
three cases:

1. override equals the configured base timeout;
2. override is shorter than the configured base timeout;
3. override is longer than the configured base timeout.

The tests must protect both transport-selection behavior and user-visible
timeout behavior.

At minimum verify:

- equal/shorter overrides choose the shared transport path;
- a longer override chooses a transport configured with the widened connect
  timeout rather than silently retaining the shorter base connector policy;
- `FetchLimits.timeout_ms` always reflects the requested override;
- normal fetch and conditional fetch both install
  `client_timeout(effective_timeout_ms)` at request level;
- DNS validation continues to derive its timeout from the effective
  `FetchLimits`;
- timeout failures continue mapping to
  `FetchError::Timeout(effective_timeout_ms)`;
- redirect hops continue using the same effective request timeout and
  per-hop validation.

Prefer a small private decision helper or internal transport-construction
seam that can be tested directly instead of relying only on source-text
static guards.

Where practical, add a deterministic loopback behavioral test with a short
base timeout and a longer override. A delayed TLS/connect-establishment
fixture is preferable because it exercises the physical connector timeout
that caused the defect. Keep wall-clock values small and tolerance broad
enough for CI stability.

If a reliable physical-connect delay cannot be produced portably, retain
direct structural tests of the selected client-construction mode plus the
existing loopback request timeout suites. Document the limitation rather
than introducing a flaky timing test.

## Work item 2 — Use hybrid timeout-client reuse

Refactor `FetchClient::with_timeout_ms` so timeout overrides preserve both
semantics and the Phase 20 performance win.

Required behavior:

~~~text
requested timeout <= base client timeout
    -> clone/reuse the existing eggfetch client
    -> update FetchLimits.timeout_ms
    -> request-level timeout supplies the stricter logical deadline

requested timeout > base client timeout
    -> construct one eggfetch client with the widened timeout
    -> preserve all base client construction policy
    -> update FetchLimits.timeout_ms
~~~

The exact equality boundary may be implemented as `<=`.

Why this split is safe:

- for a shorter request, the request-level total deadline is stricter than
  the base connector timeout, so the physical connector cannot exceed the
  requested logical timeout;
- for a longer request, the client-level physical connector timeout must also
  be widened or the old base value remains an unintended ceiling.

Centralize eggfetch client creation in one private helper used by both
`FetchClient::new` and the widened-override branch. That helper must preserve
all client-wide behavior currently configured by eggsearch, including:

- user agent;
- base timeout fields;
- redirects disabled at the transport client;
- any future client-wide limits/options added to the normal constructor.

Do not duplicate a second builder sequence that can drift from
`FetchClient::new`.

Do not change the public `with_timeout_ms` signature.

## Work item 3 — Preserve one adjusted client per batch

Keep the Phase 20 batch-level construction change.

`run_batch_fetch` should continue to resolve the effective fetch client once
before spawning item futures.

For a widened timeout this means one dedicated client/pool for the entire
batch, not one client per web item.

For an equal/shorter timeout this means the adjusted `FetchClient` shares
the base eggfetch client/pools.

Add a targeted test around batch setup so future refactors cannot move
`with_timeout_ms` back into `make_batch_fetch_future`.

The per-item future should not need to know whether the transport is shared
or dedicated.

## Work item 4 — Keep the derived-cache Arc change intact

Do not undo the Phase 20 derived-cache sharing work.

Retain:

~~~text
LruCache<DerivedCacheKey, Arc<DerivedDocumentCacheEntry>>
~~~

and the internal shared getter.

Re-run exact cache accounting, eviction, invalidation, revalidation, and
cache-hit response tests after the timeout correction because the corrective
candidate must include the complete Phase 20 behavior.

Add the missing benchmarks described below; do not redesign cache policy.

## Work item 5 — Add the missing Phase 20 performance benchmarks

Extend `benches/perf.rs` with deterministic local-only benchmarks for:

### Timeout client adjustment

Measure separately:

- equal-timeout `with_timeout_ms`;
- shorter-timeout `with_timeout_ms` using shared transport;
- longer-timeout `with_timeout_ms` using the dedicated widened client.

The purpose is to make the tradeoff explicit: common shortening/equality
should remain cheap, while widening intentionally pays client construction
cost to preserve semantics.

If direct construction requires expensive TLS setup, that cost is valid
evidence and should not be hidden.

### Derived cache hits

Benchmark:

- internal shared derived hit with a small document;
- internal shared hit around 12K extracted characters;
- internal shared hit around 50K extracted characters or another
  representative large document below configured caps;
- owned compatibility getter for the same payloads as a comparison.

The benchmark should make clear that the optimization removes deep copying
from the internal hot path, not from the intentionally owned public wrapper.

### Batch timeout setup

Benchmark a small private/setup helper if necessary so representative batch
sizes can show that timeout adjustment occurs once per batch and does not
scale client construction with item count.

Do not widen public API solely for Criterion.

## Work item 6 — Repair Phase 19 benchmark comparability

Do not describe the existing 72.7/292.7 microbenchmark values and
120.2/503.7 selector values as direct before/after timings.

Add an apples-to-apples reference benchmark for the legacy full-sort
candidate-selection algorithm using the same generated entries, query,
filters, result budget, and Criterion process as the optimized selector.

A benchmark-local legacy reference implementation is acceptable if it is
clearly named and cannot be used by production code.

At minimum compare at:

- 1,000 entries;
- 4,096 entries;
- `max_results=10` or the exact same retained-K budget for both algorithms.

The legacy reference must preserve the old stable tie behavior so the
comparison measures algorithm/work reduction rather than changed ordering.

Also retain the score-only historical microbenchmarks as lower-level
characterization; relabel their role in documentation rather than deleting
them.

Update the Phase 19 implementation record so it distinguishes:

- historical score-only measurements;
- apples-to-apples legacy-selector versus optimized-selector measurements;
- warm Arc handle characterization.

## Work item 7 — Add the missing Phase 21 discovery-cache benchmarks

Add deterministic characterization for repeated access to the cached MCP
contract.

At minimum measure:

- repeated `tool_definitions()` access on one server instance;
- repeated `tool_fingerprint()` access;
- where practical, a reference uncached decorated-contract/fingerprint build
  using an internal/benchmark-only helper.

The benchmark should not require binding a network listener.

Do not change the public MCP tool list or fingerprint algorithm to make the
benchmark easier.

Remember that `tool_definitions()` still returns an owned
`Vec<rmcp::model::Tool>`; the claim should be that schema decoration,
sorting, and fingerprint computation are no longer repeated, not that the
entire call is allocation-free.

## Work item 8 — Reconcile the Phase 19-22 evidence record

Once the corrective implementation and benchmark work are complete, update:

- `plans/performance-optimization-roadmap.md`;
- Phase 19 implementation record;
- Phase 20 implementation record;
- Phase 21 implementation record;
- Phase 22 implementation record;
- `plans/registry.md`;
- architecture/testing documentation if benchmark inventory changes.

The record must explicitly correct these earlier overstatements:

1. the initial Phase 19 timing pairs were not apples-to-apples before/after
   selector benchmarks;
2. Phase 20 originally lacked the registered derived-cache and timeout-client
   benchmark cases;
3. Phase 21 originally lacked repeated contract/fingerprint benchmark
   characterization;
4. Phase 22's local gates did not replace a seven-target release
   qualification after production/Cargo changes.

Do not erase the original implementation SHAs or historical measurements.
Add corrective evidence with the new exact candidate.

## Work item 9 — Run full local correctness and release gates

On the exact corrective implementation candidate, run at minimum:

~~~text
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
make hygiene
make packaging-check
make bench-check
make release-check
~~~

Run the targeted Criterion cases and record:

- benchmark command/filter;
- Rust version;
- target triple;
- baseline/reference median;
- corrected median;
- any statistically noisy result that should not be presented as a win.

The release gate must run from a clean committed candidate where required by
the repository's publish-dry-run checks.

## Work item 10 — Re-run the seven-target non-publishing qualification

Because phases 19-23 modify release-relevant production/dependency files, run
the existing release workflow against the exact corrective candidate:

~~~text
mode = qualify
ref  = <exact corrective candidate SHA>
~~~

Required target matrix remains:

1. `x86_64-unknown-linux-gnu`;
2. `aarch64-unknown-linux-gnu`;
3. `armv7-unknown-linux-gnueabihf`;
4. `x86_64-apple-darwin`;
5. `aarch64-apple-darwin`;
6. `x86_64-pc-windows-msvc`;
7. `aarch64-pc-windows-msvc`.

The final assembly job must pass the existing exact asset-set contract.

Record:

- workflow run ID and URL;
- `QUALIFIED_SHA`;
- package version;
- seven target results;
- assembly result;
- artifact name;
- exact 16-file assembly count;
- checksum validation;
- representative per-target sizes.

Do not publish a release as part of this corrective phase unless separately
requested.

## Work item 11 — Close only after exact evidence exists

Append an implementation record to this plan containing:

- corrective implementation SHA;
- exact qualified SHA;
- timeout semantic tests and their result;
- which timeout cases reuse versus rebuild transport;
- Phase 19 legacy-reference versus optimized selector benchmarks;
- Phase 20 timeout/cache/batch benchmarks;
- Phase 21 tool-contract/fingerprint benchmarks;
- local correctness/release gate results;
- qualification workflow ID and seven-target results;
- assembled artifact count/checksums;
- any deviations, rejected optimizations, or upstream eggfetch dependency
  considerations.

Then mark Phase 23 `implemented` and update the registry/roadmap in the same
closure change.

If a production change is made after qualification, apply the existing
exact-candidate invalidation rule and requalify the new candidate before
claiming release readiness.

## Correctness and compatibility invariants

Phase 23 must preserve:

- all ten stable MCP tools;
- MCP request/response schemas;
- public Rust API compatibility;
- local-search ranking and deterministic tie ordering;
- cache keying, byte accounting, invalidation, and freshness behavior;
- DNS/SSRF validation and resolved-address pinning;
- redirect revalidation;
- OriginController retry/backoff ownership;
- response body/character caps;
- sanitization/trust markers;
- browser/PDF support;
- provider coverage;
- stdio/Streamable HTTP integration verification;
- the explicit Tokio feature set from Phase 22;
- the Phase 18 identity-encoding workaround and eggfetch ownership boundary.

## Non-goals

Phase 23 does not:

- revert the local-search Arc/top-K optimization;
- revert derived-cache sharing;
- revert MCP projection/discovery caching;
- restore Tokio `full`;
- remove rmcp client features;
- redesign batch scheduling;
- add request coalescing/singleflight;
- modify ranking weights;
- introduce a private transport stack;
- fork or patch eggfetch inside eggsearch;
- consume unpublished eggfetch code;
- remove the Phase 18 compression workaround;
- publish a release.

## Suggested execution order

~~~text
1. add timeout-mode decision tests and behavioral fixture
2. centralize eggfetch client construction
3. implement shared-for-shorter / dedicated-for-longer timeout behavior
4. verify batch still constructs one adjusted client
5. add Phase 20 cache/timeout/batch benchmarks
6. add Phase 19 apples-to-apples legacy selector benchmark
7. add Phase 21 contract/fingerprint benchmarks
8. run targeted tests + Criterion characterization
9. run full local/release gates on a clean committed candidate
10. workflow_dispatch release-binaries.yml mode=qualify ref=<exact SHA>
11. inspect exact 16-file qualification artifact/checksums
12. reconcile phases 19-22 + roadmap + registry
13. close Phase 23 with exact evidence
~~~

## Acceptance criteria

Phase 23 is complete only when all of the following are true:

1. Equal and shorter fetch timeout overrides reuse the existing eggfetch
   transport client.
2. Longer fetch timeout overrides use a client whose physical connect timeout
   is widened to the requested value.
3. `FetchLimits.timeout_ms`, DNS validation, request-level timeout fields,
   error mapping, redirects, and conditional fetch all use the same effective
   override.
4. Client construction is centralized so normal construction and widened
   override construction cannot silently drift.
5. `batch_fetch` constructs at most one adjusted fetch client per batch.
6. The Phase 20 derived-cache Arc implementation and exact accounting remain
   intact.
7. Deterministic tests protect the timeout-mode boundary and the production
   resolved-route semantics as far as portable local fixtures allow.
8. Phase 20 has Criterion coverage for timeout adjustment, representative
   derived-cache hits, and batch timeout setup.
9. Phase 19 has an apples-to-apples legacy full-sort versus optimized
   candidate-selector benchmark on identical workloads at 1,000 and 4,096
   entries.
10. Phase 19 documentation no longer presents unlike score-only and selector
    benchmarks as direct before/after evidence.
11. Phase 21 has repeated tool-definition/fingerprint cache
    characterization.
12. MCP tool contract/fingerprint outputs remain unchanged.
13. `cargo fmt --check`, clippy, no-default check, all-feature tests,
    hygiene, packaging, bench-check, and release-check pass on the corrective
    candidate.
14. `release-binaries.yml` runs in `qualify` mode against the exact
    corrective candidate SHA.
15. All seven release targets pass.
16. Final assembly passes with the exact expected 16-file set and valid
    checksums.
17. The qualification run ID, `QUALIFIED_SHA`, target results, artifact
    evidence, and benchmark environment/results are committed to the
    planning record.
18. Phases 19-22 remain historically implemented, but their evidence records
    are reconciled with this corrective phase rather than silently rewritten.
19. Phase 23 and the performance roadmap are marked fully closed only after
    all evidence above exists.

## Handoff notes

Do not solve the timeout defect by always rebuilding the eggfetch client. That
would restore semantics but unnecessarily discard the valid Phase 20 pooling
win.

The key distinction is whether the requested timeout needs to widen a
client-scoped physical connector deadline. Shorter/equal overrides can rely on
the request-level deadline while retaining shared transport state; longer
overrides require a client built with the larger connection timeout under
eggfetch 0.1.7/current semantics.

Likewise, the missing benchmarks are evidence corrections, not an invitation
to rewrite the implementation. Keep the corrective code small and spend the
rest of this phase proving the already-landed hot-path work honestly across
the benchmark and release matrices.

## Implementation record

Not yet implemented. Record the exact corrective implementation SHA,
qualification SHA/run, timeout-mode evidence, benchmark results, local gates,
artifact checksums, and any deviations here before changing status to
`implemented`.
