# Phase 12: Performance and Dependency Optimization

## Objective

Reduce eggsearch runtime overhead, binary/dependency footprint, and response latency while preserving the agent-facing correctness work from prior phases. This phase should make eggsearch cheaper to run as a local MCP server for codegg and other harnesses, especially on laptops, small VPS instances, and long-lived daemon processes.

The focus is measured optimization, not speculative rewrites. Establish baselines, then optimize the largest costs: provider fan-out, fetch/document extraction, local workspace scanning, response serialization, optional dependencies, and CI feature matrix behavior.

## Current context

Eggsearch now exposes a rich MCP tool surface:

- search and fetch tools;
- repo and local workspace support;
- security and research workflows;
- structured warnings;
- workflow recipes and next actions;
- deterministic identity and evidence bundles;
- code-aware fetch metadata.

This richer response model increases CPU, allocation, serialization, and output-size pressure. Phase 12 should ensure the new metadata does not make common paths expensive.

## Non-goals

- Do not remove agent-facing metadata to win microbenchmarks.
- Do not introduce persistent indexing unless bounded and optional.
- Do not add large dependencies for benchmarking or profiling.
- Do not change tool semantics for performance without explicit tests.
- Do not optimize live provider latency by violating provider rate limits.

## Workstream 1: Establish benchmark and measurement baselines

### Required benchmarks

Add a lightweight benchmark suite or test-mode timing harness for:

- query planning and provider dispatch with mocked providers;
- source-card conversion and identity generation;
- suggested-fetch generation for repo/security/research;
- code-context extraction on representative files;
- evidence bundle construction;
- local workspace file classification and path filtering;
- JSON serialization of large responses;
- provider-status response with `recipe_detail = none/summary/full`.

Use Criterion only if dependency policy allows it. Otherwise, add deterministic microbench-like ignored tests or a small internal `cargo run --bin` benchmark harness behind a feature.

### Acceptance criteria

- Baseline numbers can be collected locally without network.
- Benchmarks use fixture data and mocked providers.
- Performance regressions are reviewable even if not enforced in CI initially.

## Workstream 2: Response-size budget audit

### Problem

The recent metadata additions can inflate responses. Agents benefit from structure, but output size must remain predictable.

### Required behavior

Define size budgets for major responses:

- `provider_status` summary mode;
- `web_search` with default max results;
- `repo_search` with grouped results and next actions;
- `security_search` with applicability/remediation;
- `research_search` with claims/source quality/gaps;
- `repo_fetch` with code context/span;
- `build_evidence_bundle` default limits.

Budgets can be soft documented budgets first, then tested via fixture snapshots if feasible.

### Implementation guidance

- Use explicit caps for claims/conflicts/gaps/source quality/next actions/suggested fetches.
- Avoid duplicating source-card data inside next actions.
- Avoid repeating long reason strings where reason codes suffice.
- Consider `detail` or `include_*` knobs only when default bloat is significant.

### Tests

- Provider status summary response is smaller than full response.
- Next-action templates do not include full source bodies.
- Research metadata caps are enforced.
- Evidence bundle truncation obeys configured max chars.

## Workstream 3: Dependency feature audit

### Problem

Eggsearch has optional heavier behavior: PDF extraction, HTTP client, scraper/HTML parsing, schema generation, local workspace, security parsing, and code evidence. Some deployments may want a smaller binary.

### Required behavior

Audit `Cargo.toml` features and dependencies:

- identify default vs optional dependencies;
- ensure PDF support is optional and documented;
- consider feature-gating heavy HTML/PDF/code extraction if practical;
- verify `--no-default-features` builds and tests meaningful minimal surface;
- consider splitting dev-only dependencies from runtime dependencies;
- ensure feature combinations used in CI are valid.

### Tests/verification

- `cargo tree -e features` inspected and summarized in docs or plan notes.
- `cargo test --no-default-features` passes or documented exceptions are fixed.
- `cargo test --all-features` passes.
- No accidental runtime dependency is pulled only for tests/docs.

## Workstream 4: Provider dispatch and concurrency tuning

### Required behavior

Preserve true bounded dispatch while reducing overhead:

- verify no spawn-all behavior regressed;
- avoid unnecessary allocation per job;
- keep deterministic output ordering;
- keep per-provider/global concurrency caps;
- measure effect of subquery count and provider count;
- make timeout/deadline accounting cheap and accurate.

### Potential optimizations

- Preallocate result/failure vectors when job count known.
- Use small-vector-like patterns only if dependency-free or already present.
- Avoid cloning large query strings unnecessarily.
- Reuse normalized provider metadata across jobs.

### Tests

- Existing concurrency tests remain green.
- High subquery/provider fixture does not exceed caps.
- Deadline behavior remains deterministic.

## Workstream 5: Local workspace scan optimization

### Required behavior

Local workspace search should stay bounded and responsive:

- honor `max_indexed_files` strictly;
- skip directories before expensive metadata reads where possible;
- avoid reading file bodies when path/language filters can reject them;
- avoid binary reads using extension and cheap byte sniffing;
- stop promptly on timeout/deadline;
- keep git metadata discovery bounded.

### Tests

- Large fixture tree respects file cap.
- Skip dirs are not descended into.
- Binary files are not read as text.
- Timeout returns partial result with warning/state.
- Symbol search does not read more files than necessary under caps.

## Workstream 6: Serialization and allocation review

### Required behavior

Audit common hot paths for avoidable cloning and allocation:

- source-card conversion;
- warning conversion/dedup;
- identity key building;
- next-action construction;
- evidence bundle source/fetch dedupe;
- research analysis source-quality scans;
- security remediation generation.

Use mechanical improvements only where readability remains acceptable.

### Tests

Existing behavior tests should cover output equality. Add targeted tests if dedupe/order semantics change.

## Workstream 7: Release profile and binary size

### Required behavior

Review release settings:

- LTO/codegen/strip settings;
- panic strategy if appropriate;
- debug symbols for release-debug profile if needed;
- crate feature docs for minimal/headless deployment.

Collect baseline binary sizes for:

- default release;
- all-features release;
- no-default-features release if supported.

Do not optimize binary size by making defaults surprising.

## Workstream 8: CI performance and feature matrix

### Required behavior

Update CI from prior polish pass if needed:

- add `cargo test --no-default-features` if not already present;
- consider `cargo check --no-default-features`;
- keep CI runtime reasonable;
- avoid live-network tests in normal CI;
- ensure mock/corpus tests run in a deterministic mode.

### Acceptance criteria

- CI validates all important feature sets.
- CI remains fast enough for normal PRs.
- Live smoke tests remain opt-in/ignored.

## Workstream 9: Documentation

Update:

- README performance/deployment section;
- `AGENTS.md` verification matrix;
- feature flag docs;
- benchmark invocation instructions;
- response-size/detail control docs.

## Acceptance criteria

- Baseline benchmark or measurement harness exists.
- Major response sizes are capped or documented.
- Dependency features are audited and CI-verified.
- Local workspace and dispatch paths remain bounded under stress fixtures.
- Serialization/clone hot spots are reviewed and improved where low risk.
- CI includes `--no-default-features` verification unless explicitly impossible.
- Release/deployment docs explain default vs minimal builds.
