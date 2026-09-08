# Phase 11 — Architecture and Workflow Consolidation

Status: complete
Depends on: none
Baseline for planning: `4a713ff82cec701534e285bbe3d330ae121f352c`
Roadmap: `plans/maintenance-codegg-quality-roadmap.md`

## Objective

Reduce architectural concentration and duplicated orchestration while preserving the existing ten-tool MCP contract and behavior. Split the two largest coordination modules, partition the mega integration test suite, and introduce narrow shared workflow primitives used by repo, research, and security search paths.

This is a behavior-preserving refactor phase. It should lower maintenance cost before any additional feature work lands.

## Non-goals

- No new MCP tools.
- No new search providers.
- No intentional request/response breaking changes.
- No replacement of typed repo/research/security models with generic JSON or stringly-typed abstractions.
- No broad performance rewrite unless needed to preserve existing behavior after extraction.

## Invariants

1. Stable MCP tool names, schemas, trust semantics, IDs, retrieval summaries, and next-action behavior remain compatible.
2. `make check` remains the routine gate and stays network-free.
3. Refactors should move code behind existing public/internal seams before changing behavior.
4. Shared workflow code must express common mechanics, not domain policy.
5. Explicit provider selection, capability enforcement, cooldown behavior, and failure accounting remain unchanged.
6. SourceCard normalization and stable identity rules must remain byte-for-byte compatible where currently covered by contract tests.

## Production changes

### 1. Split `src/mcp/tools.rs`

Create `src/mcp/tools/` and move each tool handler/schema surface into behavior-oriented modules, for example:

```text
src/mcp/tools/
  mod.rs
  web_search.rs
  web_fetch.rs
  batch_fetch.rs
  provider_status.rs
  repo_search.rs
  repo_fetch.rs
  repo_map.rs
  security_search.rs
  research_search.rs
  evidence_bundle.rs
  common.rs
```

`mod.rs` should register/export the same tool surface. Cross-tool helpers belong in `common.rs` only when they are genuinely shared; avoid recreating a monolith there.

Acceptance target: no individual tool module should own unrelated domain execution logic merely because the MCP handler calls it.

### 2. Split `src/meta/adapter.rs`

Audit responsibilities in `adapter.rs` and extract coherent units such as provider invocation, response adaptation, error classification, and result normalization. Exact filenames should follow the actual implementation discovered during handoff.

Keep the existing adapter-facing API stable where practical so downstream refactors can be incremental.

### 3. Introduce a narrow shared workflow substrate

Extract reusable primitives for the mechanics common to repo/research/security discovery. Candidate concepts include:

```text
PlannedLane<TPolicy>
WorkflowExecution
RetrievalAttemptSet
NormalizedLaneResults
EvidenceGroup<K>
CoverageSummary
FetchCandidateSet
```

Do not require every workflow to implement every stage. Traits should be small and capability-oriented.

The first consolidation targets should be duplicated operations around:

- bounded subquery/lane execution;
- retrieval-attempt recording;
- SourceCard collection/deduplication;
- fetch-candidate construction boilerplate;
- ranking/diversity invocation;
- next-action assembly where semantics are identical.

Repo-specific structured locators, security advisory synthesis, and research source-role taxonomy should remain domain modules.

### 4. Consolidate suggested-fetch machinery

`repo`, `research`, and `security` suggested-fetch code should share helper constructors/builders for `FetchCandidate` and common ranking conversion behavior.

Prefer typed builders such as:

```text
FetchCandidateBuilder::from_card(...)
  .group(...)
  .structured_repo_fetch(...)
  .recommended_extract_mode(...)
```

over repeated full struct literals.

Retain specialized synthetic security candidates for authoritative advisory URLs, but route them through the same bounded builder/ranking path.

### 5. Partition `tests/integration.rs`

Move tests into behavior-oriented suites. Suggested categories:

```text
tests/mcp_tools.rs
tests/web_search_integration.rs
tests/repo_workflow.rs
tests/research_workflow.rs
tests/security_workflow.rs
tests/provider_routing.rs
tests/evidence_contract.rs
```

Do not split only by file size; use stable behavioral ownership. Shared fixtures may move into `tests/support/` if this reduces duplication.

Historical phase-named suites may remain temporarily, but newly moved tests should use behavior names.

### 6. Add architectural guardrails

Add lightweight static/contract tests to discourage recurrence. Useful guards include:

- stable tool registration count/name checks;
- no direct domain orchestration in MCP transport-only modules where a service seam exists;
- optional source-size warning script/report for exceptional files, not a brittle hard cap on all Rust files;
- common workflow invariants tested once and domain-specific invariants tested in their own suites.

## Verification

Required:

```text
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features
./packaging/check-contract.sh
```

Also run focused contract tests before and after refactoring and compare serialized fixtures/stable IDs where applicable.

## Acceptance criteria

- `src/mcp/tools.rs` is replaced by a modular tool directory with unchanged stable tool registration.
- `src/meta/adapter.rs` is decomposed into coherent modules; no replacement god module is introduced.
- repo/research/security workflows consume at least one shared execution primitive and one shared fetch-candidate construction/ranking primitive.
- domain-specific semantics remain in typed domain modules.
- `tests/integration.rs` is substantially reduced or removed, with tests partitioned by behavioral contract.
- no stable MCP schema/tool-name regression is observed.
- `make check` passes on the exact candidate.

## Handoff notes

Perform this phase as a sequence of small mechanical commits where possible: MCP split, adapter split, workflow primitive extraction, suggested-fetch consolidation, test partition. Avoid mixing semantic changes into extraction commits because later phases depend on being able to distinguish refactor regressions from new behavior.
