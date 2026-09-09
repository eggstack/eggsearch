# Phase 15 — Public API, Documentation, Tests, and Repository-Hygiene Closure

Status: implemented
Depends on: phases 11-14
Baseline for planning: `4a713ff82cec701534e285bbe3d330ae121f352c`
Roadmap: `plans/maintenance-codegg-quality-roadmap.md`

## Objective

Close the consolidation workstream by making the intended Rust API boundary explicit, removing repository artifacts and historical maintenance debt, reconciling documentation with the implemented capability model, and running a final cross-workstream qualification pass.

This phase is not a grab bag for new features. It exists to make the post-refactor repository easier to maintain and safer to evolve.

## Current problems

`lib.rs` describes eggsearch as a single-binary MCP application while publicly exporting broad implementation modules (`core`, `fetch`, `integrations`, `mcp`, `meta`, `platform`, `startup`, `update`). That creates an unclear semver commitment for Rust consumers.

Tests and docs also retain historical phase naming that is useful for archaeology but weaker as a long-term behavioral contract. The root `typescript` file is an accidental ANSI terminal transcript from a prior test run and should not be versioned.

Documentation already has contract tests, but provider-capability prose can still drift when the same claim is maintained in several places.

## Non-goals

- No new MCP tools/providers/workflows.
- No major CLI redesign.
- No blanket rewrite of all documentation for style alone.
- No arbitrary source-file-size policy that blocks legitimate complex modules without architectural evidence.

## Invariants

1. Stable MCP and CLI compatibility remains intact unless an intentional Rust-library API change is explicitly documented as such.
2. Generated documentation should not become a build-time network dependency.
3. Repository hygiene checks must be deterministic and portable across supported development platforms where feasible.
4. Historical planning documents remain available; do not rewrite completed phase history merely to make it look current.
5. Final closure evidence is recorded against the exact candidate commit.

## Production/API changes

### 1. Decide and enforce the Rust-library contract

Perform a public-item audit after phase 11 has stabilized module boundaries.

Choose one of two explicit policies:

#### Policy A — application-first crate

Expose only intentionally reusable request/result/config/client/server types and make implementation modules `pub(crate)` where possible. Introduce a narrow public facade module if needed.

#### Policy B — supported Rust library

Document supported modules/types as a real semver surface, add library examples, and mark internal/unstable modules clearly. Avoid exposing deployment/update/integration internals merely because they are convenient to test.

The implementation agent should prefer Policy A unless there is evidence of real downstream Rust-library use or a deliberate project decision to support Policy B.

Do not make large breaking visibility changes without first checking crates.io/downstream references and documenting migration impact.

### 2. Remove accidental artifacts

Delete the root `typescript` terminal transcript and audit the root/tree for similar generated logs, editor dumps, test transcripts, temporary files, and release artifacts.

Update `.gitignore` where a stable pattern can prevent recurrence without masking legitimate source files.

### 3. Add repository-hygiene checks

Add a deterministic script/test such as `packaging/check-repo-hygiene.sh` or equivalent invoked by `make check` if sufficiently portable.

Checks may include:

- known forbidden transcript/log artifact names/patterns;
- ANSI terminal escape sequences in unexpected root text artifacts;
- tracked build output directories/files;
- oversized unexpected root blobs requiring an allowlist;
- stale generated temporary files.

Keep the allowlist explicit and small. Do not reject documentation/corpus fixtures solely because they are large.

### 4. Rename/repartition historical tests

Where tests named after implementation phases have become permanent regression coverage, move/rename them to behavioral contract names. Preserve regression fixture provenance in comments where useful.

Examples:

```text
phase1_provider_contract -> provider_request_contract
phase2_extract_fetch -> extract_fetch_contract
phase5_closure -> provider_workstream_regression
security_applicability_phase8 -> security_applicability_contract
```

Exact naming should follow what each suite actually protects after phases 11-14.

### 5. Make documentation inventories derive from stable code where practical

Audit duplicated tables for:

- provider IDs/capabilities;
- stable MCP tool names;
- feature gates;
- release targets;
- client integrations.

Prefer contract tests or small generated snippets from stable descriptors over manually duplicated factual inventories. Do not generate narrative documentation from code.

### 6. Reconcile all architecture/user docs

Audit at least:

```text
README.md
architecture/overview.md
architecture/meta.md
architecture/mcp.md
architecture/local-workspace.md
architecture/testing.md
docs/features.md
docs/tool-matrix.md
docs/provider-setup.md
docs/codegg-integration.md
docs/test-inventory.md
AGENTS.md
```

Update for:

- modular MCP/workflow architecture;
- real provider probing;
- structured local code intelligence;
- enriched repo map;
- focused batch retrieval;
- intentional Rust public API;
- current test inventory/counts;
- current provider capability enforcement semantics.

### 7. Add maintenance architecture guidance

Document ownership boundaries and extension rules so new features do not recreate the previous concentration. Include guidance such as:

- new MCP tools get dedicated modules;
- provider adapters do not own workflow policy;
- domain workflows use shared execution primitives but retain typed domain semantics;
- new providers require an evidence-class/capability justification;
- new local parser backends implement bounded capability interfaces;
- new integration tests belong to behavior-oriented suites.

### 8. Final CodeGG contract qualification

Using the current CodeGG contract fixtures/integration docs, verify that the closure candidate supports the expected workflow:

```text
provider_status
repo_map
repo_search
batch_fetch/repo_fetch
research_search/security_search as applicable
build_evidence_bundle
```

Both stdio and persistent loopback HTTP integration should retain the required `web_search` and `web_fetch` tools and recommended extended tool coverage.

No CodeGG source-code changes belong in this phase; if a downstream change is necessary, write a separate CodeGG plan.

## Final verification

Required:

```text
make check
make docs-check
make bench-check
```

Run relevant fuzz smoke targets and explicit live/provider/browser/native-forge smokes according to environment availability. Release packaging need not be rebuilt for a documentation-only closure commit unless production code changed after the last qualified candidate; if it did, run the appropriate release smoke contract.

Update `docs/test-inventory.md`, plan registry status, and closure evidence against the exact final commit.

## Acceptance criteria

- the Rust public API policy is explicitly chosen, documented, and reflected in visibility/facade structure;
- accidental root artifacts including `typescript` are removed;
- deterministic repository-hygiene checks prevent common recurrence;
- permanent regression tests use behavior-oriented naming/ownership rather than historical phase labels where practical;
- duplicated factual documentation is contract-tested or code-derived where practical;
- architecture/docs accurately describe phases 11-14 behavior and no known capability contradictions remain;
- CodeGG integration contracts pass across required tools and both supported MCP transports;
- maintenance extension rules are documented;
- `make check`, docs checks, and targeted closure qualification pass on the exact candidate;
- `plans/registry.md` marks phases 11-15 implemented only after their individual acceptance criteria have been exercised.
