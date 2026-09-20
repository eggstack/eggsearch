# Phase 22 — Dependency Footprint Qualification and Performance Closure

Status: planned
Depends on: phases 19-21
Baseline for planning: `205ab26fb03c6769035a1c05bb9b1f41c2a9ead1` (`main`)
Governing roadmap: `performance-optimization-roadmap.md`

## Why this phase exists

The preceding phases target high-confidence runtime inefficiencies. This final phase qualifies dependency/build footprint opportunities and closes the campaign with reproducible evidence.

The direct Tokio dependency currently enables `features = ["full"]`. That is convenient but broader than eggsearch's visibly used runtime surface. Narrowing it may reduce compile/link footprint and dependency activation, but only if every supported build mode continues to compile and all runtime features remain available.

The rmcp client-related features are different. The audit initially made them look removable from the server binary, but `src/integrations/common.rs` actively uses:

- `TokioChildProcess` and rmcp client support to verify stdio integrations;
- `StreamableHttpClientTransport` and rmcp client support to verify HTTP integrations after `integrate --apply`.

Those features therefore implement a supported capability. This phase must treat them as required unless an equivalent supported verification path exists without creating an eggsearch-owned MCP client.

This phase also records the final performance and footprint evidence for phases 19-21 and establishes regression guards where the evidence justifies them.

## Objective

Minimize direct runtime feature activation where mechanically safe, explicitly retain features that support real behavior, and close the performance campaign with exact correctness, dependency, binary-size, and benchmark evidence.

The desired end state is:

- Tokio no longer uses the blanket `full` feature if an explicit smaller set supports every build/test/runtime mode;
- any retained broad feature is justified by evidence rather than assumption;
- rmcp client/child-process/HTTP-client features are documented as capability-bearing and retained unless a supported equivalent proves otherwise;
- no provider/fetch/browser/PDF/integration capability is lost for size;
- normal dependency graphs and representative release-binary sizes are recorded before/after;
- performance benchmarks from phases 19-21 are rerun on one exact closure candidate;
- release/packaging checks pass on that candidate.

## Work item 1 — Capture the exact pre-change dependency and binary baseline

Before manifest edits, record against the exact candidate:

~~~text
cargo tree --locked -e features
cargo tree --locked -e features --no-default-features
cargo tree --locked -e features --all-features
cargo build --locked --release
~~~

Also capture targeted inverse trees for:

- `tokio`;
- `rmcp`;
- `reqwest`;
- `eggfetch-core`;
- TLS/compression packages materially affecting the normal graph.

Record:

- exact SHA;
- Rust version;
- Cargo version;
- target triple;
- default-feature release binary size;
- all-feature release binary size if that is a meaningful supported artifact;
- relevant normal dependency counts if reproducibly obtainable.

Do not compare sizes from different targets/toolchains and call the delta meaningful.

## Work item 2 — Derive Tokio features from production use

Audit production source and feature-gated source for direct Tokio APIs.

Expected categories include, but are not limited to:

- runtime/macros;
- multi-thread runtime where used by `#[tokio::main]`;
- time;
- sync;
- net;
- signal;
- process;
- async I/O utilities.

Do not hard-code that list from the plan. Derive it from the current source and compile matrix.

Replace `features = ["full"]` with the smallest explicit direct feature set that satisfies all supported builds.

After each narrowing attempt run at minimum:

~~~text
cargo check --locked --no-default-features
cargo check --locked
cargo check --locked --all-features
cargo test --locked --all-features
~~~

Also exercise feature-specific tests for browser/PDF paths where compile coverage alone is insufficient.

If transitive dependencies still enable a Tokio feature, that does not invalidate the direct-dependency cleanup. The goal is to stop eggsearch itself from requesting unused features, while recording the effective graph honestly.

## Work item 3 — Preserve rmcp integration-verification capabilities

Do not remove rmcp's client-related features merely to eliminate transitive reqwest.

The current supported `integrate --apply` flow verifies the installed server through rmcp:

- stdio via `TokioChildProcess`;
- Streamable HTTP via `StreamableHttpClientTransport`;
- both via rmcp client `ServiceExt` operations and `tools/list`.

These checks validate actual MCP interoperability rather than only process/HTTP liveness.

Retain:

- rmcp client support;
- child-process transport support;
- Streamable HTTP client support;

unless the current rmcp version exposes a narrower feature combination that still provides those exact APIs.

Do not replace them with a hand-written JSON-RPC/MCP verifier as a footprint optimization. That would increase maintenance burden and duplicate protocol logic.

If a narrower rmcp feature split exists, qualify it through the same integration tests. Otherwise document the existing features as intentional.

## Work item 4 — Audit other direct feature sets only where evidence is strong

Review direct dependencies with broad/default feature activation, especially large transport/parser stacks.

Candidates may include:

- TLS;
- compression;
- browser support;
- PDF support;
- serde/schema tooling.

Rules:

- do not remove a feature merely because a static search finds no direct symbol; proc-macro/generated/feature-gated use must be considered;
- optional browser/PDF features remain available exactly as documented;
- Phase 18 requires the current eggfetch compression feature budget and workaround boundaries unless separately superseded;
- no new feature-specific binary SKU is introduced;
- changes should reduce direct activation without requiring compatibility shims.

If the audit finds no further safe reductions beyond Tokio, record that conclusion rather than manufacturing work.

## Work item 5 — Add manifest/feature regression guards where justified

If Tokio is narrowed successfully, add a deterministic guard preventing accidental return to `features = ["full"]`.

Retain and update the existing eggfetch feature-budget guard.

If rmcp's retained feature set is intentional, add or improve a comment/static test tying those features to `integrations/common.rs` verification behavior so a future cleanup does not remove them based on the mistaken assumption that eggsearch is server-only.

Avoid brittle tests that duplicate Cargo parsing poorly. Prefer existing repository guard conventions.

## Work item 6 — Rerun the performance benchmark set on one closure candidate

On one exact candidate containing phases 19-21, run the targeted benchmark set covering:

- local inventory candidate selection;
- warm cached inventory acquisition/search;
- timeout-adjusted fetch-client construction;
- derived-cache hit paths;
- representative compact/standard MCP projection;
- repeated tool-contract/fingerprint access.

Record environment and compare to each phase's pre-change baseline where comparable.

Do not aggregate unrelated microbenchmarks into one synthetic percentage.

If a claimed optimization shows no measurable benefit and materially increases complexity, revert or simplify it before closure.

## Work item 7 — Run full correctness/packaging qualification

Run the repository's normal gates on the exact closure candidate:

~~~text
make check
make packaging-check
make release-check
make bench-check
~~~

If release-relevant production/dependency files changed after the last seven-target qualification, follow the repository's existing release rule and run the appropriate non-publishing release qualification before treating the candidate as release-ready.

This phase does not publish a release unless separately requested.

## Work item 8 — Record final footprint evidence

After all accepted feature changes:

- rerun the same cargo-tree commands as Work item 1;
- rebuild with the same toolchain/target/profile;
- record before/after binary sizes;
- identify which dependency/features disappeared or remained;
- explicitly state when an expected dependency remains because a supported capability requires it.

In particular, if reqwest remains transitively through rmcp, record that this is expected because HTTP integration verification uses rmcp's Streamable HTTP client transport.

Do not claim transport consolidation is incomplete merely because that intentional rmcp boundary remains.

## Work item 9 — Close the workstream documentation

Update:

- `performance-optimization-roadmap.md`;
- phases 19-22 implementation records/status;
- `plans/registry.md`;
- architecture/build or maintenance documentation if the final feature contract changed;
- relevant test inventory/benchmark documentation.

The closure record should summarize:

- accepted runtime optimizations;
- measured evidence;
- rejected/deferred ideas;
- direct feature changes;
- retained rmcp feature rationale;
- final gate results;
- exact closure SHA.

## Correctness/capability invariants

Preserve:

- all ten MCP tools;
- stdio and Streamable HTTP server operation;
- `integrate --apply` verification for supported clients/transports;
- browser/PDF feature availability;
- all provider classes;
- eggfetch transport/security boundaries;
- startup/update/deployment commands;
- public Rust API compatibility;
- release target matrix and installers.

## Required tests

At minimum:

1. no-default/default/all-feature compile matrix;
2. all-feature tests;
3. integration rendering tests;
4. stdio verification path coverage;
5. HTTP verification path coverage;
6. static guard for accepted Tokio feature policy;
7. retained eggfetch feature-budget guards;
8. targeted tests added by phases 19-21;
9. packaging/release contract tests.

## Non-goals

Phase 22 does not:

- remove rmcp client verification capability to shrink the graph;
- implement a private MCP client;
- replace Tokio;
- remove browser or PDF support;
- add size-optimized release profiles that trade away latency/correctness;
- enable panic=abort without a separate semantic review;
- remove compression features required by Phase 18;
- create per-feature release binaries.

## Suggested execution order

~~~text
1. capture pre-change dependency/size baseline
2. derive direct Tokio feature requirements
3. narrow Tokio and run compile/test matrix
4. audit rmcp and document required client features
5. inspect other direct feature sets; change only evidence-backed cases
6. add/update manifest guards
7. rerun phases 19-21 benchmarks on exact candidate
8. run make check / packaging / release / bench gates
9. capture post-change dependency/size evidence
10. update roadmap, phase records, registry, and architecture docs
~~~

## Acceptance criteria

Phase 22 is complete only when:

1. pre-change dependency graphs and representative release-binary sizes are recorded.
2. Tokio direct features are mechanically derived from the current source/build matrix.
3. `features = ["full"]` is removed if a smaller explicit set passes every supported build/test path; otherwise the blocking requirement is documented.
4. no supported runtime capability is lost from feature narrowing.
5. rmcp client/child-process/Streamable-HTTP-client features are either retained with explicit integration-verification rationale or narrowed only with equivalent capability proven.
6. no eggsearch-owned MCP client/protocol shim is introduced.
7. browser/PDF/provider/deployment capability remains unchanged.
8. accepted feature changes have deterministic regression guards where appropriate.
9. phases 19-21 targeted benchmarks are rerun on one exact closure candidate.
10. measured results are recorded without unsupported cross-machine percentage claims.
11. post-change dependency graphs and binary sizes are recorded with the same toolchain/target/profile.
12. any retained transitive reqwest path is correctly attributed to rmcp verification behavior.
13. `make check`, `make packaging-check`, `make release-check`, and `make bench-check` pass.
14. release qualification is rerun if required by the repository's exact-candidate release rule.
15. roadmap, phase records, and registry are updated together at closure.

## Handoff notes

This phase is intentionally conservative. Feature pruning can look attractive in `cargo tree` while silently removing a CLI/integration capability that is not exercised by the main server loop.

The rmcp client features are a concrete example: they are used by integration verification and should be treated as purposeful unless rmcp itself provides a narrower feature split.

The highest-confidence footprint change is likely replacing direct Tokio `full` with an explicit set. Everything else should earn its place through the compile/test matrix and measured linked output.

## Implementation record

Not yet implemented. Record exact implementation/closure SHA, feature graphs, binary sizes, benchmark evidence, commands, tests, release qualification if required, and rejected/deferred dependency changes here before changing status to `implemented`.
