# Plan 007 — Maintenance Decomposition and Overlap Ratchet

Status: implementation plan
Scope: eggsearch internal maintainability after tool-surface consolidation
Can run after: Plan 002 canonicalization helpers are stable

## Objective

Reduce maintenance risk in the remaining high-density orchestration modules and add structural guards that prevent future feature work from rebuilding monoliths or duplicating workflow mechanics. Preserve behavior and public contracts; this is a responsibility decomposition, not an architectural rewrite.

## Current-state observations

The repository has strong ownership documentation and a module-size guard for tool/adapter modules, but several implementation modules remain very large. Current mainline examples include approximately:

- `src/meta/dispatch.rs` — ~96 KB;
- `src/meta/forge_adapter.rs` — ~97 KB;
- `src/meta/local_backend.rs` — ~97 KB;
- `src/meta/evidence_bundle.rs` — ~73 KB;
- `src/meta/dependency_parse.rs` — ~60 KB;
- `src/meta/local_inventory_cache.rs` — ~60 KB;
- `src/meta/fetch_ranking.rs` — ~49 KB;
- `src/meta/local_inventory.rs` — ~50 KB;
- `src/meta/local_symbols.rs` — ~42 KB.

Large files are not automatically defects, and the existing test coverage substantially reduces correctness risk. The problem is change locality: adding a provider, workflow rule, local-code feature, or evidence field can require edits inside files that own several distinct responsibilities.

## Non-goals

- Do not split files merely to satisfy arbitrary LOC numbers.
- Do not introduce generic abstractions that erase typed repo/research/security semantics.
- Do not move provider parsing into workflow modules or workflow policy into engine adapters.
- Do not change public MCP request/response behavior as part of decomposition.
- Do not combine this plan with broad dependency or async-runtime rewrites.

## Workstream A — Expand structural metrics

Extend the existing `static_guards.rs` philosophy beyond MCP tools/adapters.

Track at least:

- source file line count;
- file byte count;
- top-level function/impl count where practical;
- number of unrelated domain imports/modules referenced;
- duplicate helper names/patterns across workflow modules where statically detectable.

Use warning thresholds first. Hard-fail only for clear regressions such as a previously-decomposed file growing back above an agreed ceiling.

Suggested warning thresholds:

- >1,600 LOC for orchestration modules;
- >80 KB for any ordinary source file;
- >25 substantial top-level functions in one orchestration file.

Exceptions must be explicit and documented for generated/fixture-like code, not silently ignored.

## Workstream B — `meta/dispatch.rs`

Audit responsibilities and extract by behavior. Candidate boundaries:

```text
src/meta/dispatch/
  mod.rs              # public orchestration seam
  execution.rs        # bounded concurrent dispatch mechanics
  provider_attempt.rs # attempt/error/outcome recording
  merge.rs            # result combination/dedup/RRF coordination
  deadlines.rs        # timeout/deadline accounting
  telemetry.rs        # dispatch telemetry construction
```

Exact names should follow discovered responsibilities, not this example mechanically.

Keep the public call surface stable so domain planners do not need to change simultaneously.

Acceptance for this module is improved change locality: provider execution mechanics, deadline logic, and result aggregation should not require editing one 90+ KB file.

## Workstream C — forge adapter decomposition

Split `meta/forge_adapter.rs` along host-independent versus host-specific mechanics.

Candidate boundaries:

- locator/ref normalization;
- URL construction and validation;
- HTTP/native forge execution;
- response parsing/tree representation;
- host-specific GitHub/GitLab/Gitea/Forgejo/Codeberg behavior.

Do not duplicate SSRF/credential/redirect policy in per-host modules. Keep shared safety checks in one owner.

Add tests ensuring all hosts retain identical security invariants after extraction.

## Workstream D — local code intelligence decomposition

`local_backend.rs`, `local_inventory.rs`, `local_inventory_cache.rs`, and `local_symbols.rs` form a coherent subsystem but currently distribute scanning, indexing, caching, ranking, and symbol behavior across large files.

Establish an explicit subsystem layout, for example:

```text
src/meta/local/
  mod.rs
  backend.rs
  inventory.rs
  cache.rs
  scan.rs
  ranking.rs
  symbols/
    mod.rs
    structured.rs
    regex.rs
```

Preserve current constraints:

- no workspace code execution;
- bounded file size/work budgets;
- root containment/symlink safety;
- structured symbol backend with regex fallback;
- budget breaches degrade to partial/fallback evidence rather than uncontrolled failure.

Avoid introducing a second cache abstraction; reuse the existing cache semantics.

## Workstream E — dependency parsing

Split `dependency_parse.rs` by ecosystem parser only if the current function structure demonstrates clean boundaries.

A likely layout:

```text
src/meta/dependency/
  mod.rs              # common normalized dependency record + dispatch
  cargo.rs
  npm.rs
  go.rs
  python.rs
  ruby.rs
  composer.rs
  maven.rs
  dotnet.rs
  containers.rs
  github_actions.rs
```

All parsers should emit one normalized internal representation. Shared path/size/root validation remains outside ecosystem-specific parsers.

Add corpus parity tests before and after extraction to prove no parser behavior changed.

## Workstream F — evidence bundle and fetch ranking

Audit overlap among:

- evidence-bundle source normalization;
- suggested-fetch construction;
- fetch candidate ranking;
- trust/evidence role aggregation;
- gap/retrieval metadata packaging.

The current architecture already names `FetchCandidateBuilder` and shared workflow attempt primitives as intended owners. Consolidate duplicate candidate normalization/scoring helpers behind those existing owners rather than introducing another abstraction.

`evidence_bundle.rs` should own deterministic handoff packaging, not search/fetch ranking policy.

`fetch_ranking.rs` should own ranking math and candidate ordering, not domain-specific source semantics.

Add static or unit guards preventing domain planners from bypassing the shared builder where the maintenance contract already requires it.

## Workstream G — canonical validation helper audit

After Plan 002 adds canonical schema translation, search for repeated implementations of:

- enum/alias parsing;
- freshness parsing;
- repository locator normalization;
- workflow/goal canonicalization;
- source-set expansion;
- provider-routing validation;
- max-results/budget clamping;
- stable warning construction.

Each behavior should have one owner where semantics are genuinely identical. Do not over-deduplicate domain-specific validation just because code looks similar.

Add tests at the shared helper seam so domain tool tests can focus on domain behavior.

## Workstream H — dead/compatibility path audit

Classify `#[allow(dead_code)]`, compatibility aliases, old parser paths, and historical fallback logic into:

- required compatibility;
- test-only;
- feature-gated but supported;
- removable.

For each compatibility path retained, document the exit condition. Do not remove code solely based on lack of default-path references if it is part of a supported feature flag or protocol fallback.

## Execution strategy

Perform this work in narrow behavior-preserving slices. For each target module:

1. lock existing behavior with focused tests;
2. extract one responsibility without semantic edits;
3. run focused tests;
4. run `make check` at milestone boundaries;
5. only after extraction, consider small duplication cleanup.

Do not combine multiple major module splits in one commit if doing so makes review provenance unclear.

## Acceptance criteria

- Existing public MCP/CLI contracts are unchanged by this plan.
- No ordinary source file remains above the agreed high-density threshold without an explicit documented exception.
- `dispatch`, forge behavior, and local-code intelligence have clear submodule ownership boundaries.
- Dependency ecosystem parsing has a single normalized output contract.
- Evidence packaging no longer owns ranking mechanics that belong to shared fetch-ranking/workflow primitives.
- Repeated canonical schema/locator/workflow validation uses shared owners where semantics are identical.
- Structural guards prevent the decomposed files from silently growing back into monoliths.
- All existing all-feature, property, fault-injection, forge-safety, local-filesystem, and parser tests remain green.

## Verification

At minimum:

```bash
make check
make docs-check
make bench-check
cargo test --all-features --test forge_adapter
cargo test --all-features --test property_local_fs
cargo test --all-features --test property_local_fs_extended
cargo test --features mock provider_workstream_regression
```

Run ecosystem-specific dependency/applicability corpora after moving each parser.

## Closure evidence

The closure report should include:

- before/after file-size/LOC table;
- responsibility map for extracted modules;
- removed duplicate helper count with justification;
- compatibility/dead-code inventory and exit conditions;
- full verification results.