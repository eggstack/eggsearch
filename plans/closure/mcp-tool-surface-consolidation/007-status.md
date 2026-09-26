# Tool-Surface Consolidation M007 — Maintenance Decomposition Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/007-maintenance-decomposition-and-overlap-ratchet.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: `08f72ab0` (dependency evidence M010 closure) plus M006 closure `85a44b7` in this sequence.

Hard dependency: M001 canonicalization seams stable (`4b7e725`,
`b1e6ea7`); satisfied. M006 closed in the same sequence and does not
block M007.

Implementation commits:

- `6b61fa053bd1a5440a781a7d154a8dc5dfae6153` — decompose `dependency_parse.rs` into per-ecosystem submodules behind normalized dispatch
- `9be8a6f288ce9540bf04998705637a7523cba8d1` — decompose `dispatch.rs` into `dispatch/` types vs execution
- `f5da11e193a673ba6ebc31a4b0ce9d234ad9fd73` — expand static guards into 007 overlap ratchet (no production behavior change)
- `0ef33712f1cd3b5240678a7a8e63d7ac6c556f62` — record 007 decomposition with responsibility map and ratchet (local facade, maintenance table)
- `fbf8457a21dc6d586ba5f903903278b3dfc8ce24` — extend ratchet to `research_workflow.rs` and `adapter/tests.rs` (this sequence)

Closure candidate: `85a44b7528cf4b63998f249206040951eaeb9d12` (M006
closure; no production delta from `fbf8457` for M007 guards — M006
closure adds only the 006 record plus roadmap/registry status).

## 1. Executive finding

M007 is complete as a responsibility decomposition with structural
guards, not an architectural rewrite. Public MCP/CLI contracts are
unchanged. Dispatch and dependency parsing are physically decomposed
with stable seams; forge, local, evidence, and fetch-ranking ownership
is enforced by fail-closed guards with explicit ratchet ceilings and
next-slice notes where physical moves are deferred. Duplicate canonical
validation now has one owner; domain planners keep typed semantics and
are forbidden from over-deduplication. All focused and broad gates pass
on the exact candidate.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| Public MCP/CLI contracts unchanged | `mcp_tools`, `evidence_contract`, `repo/research/security_workflow`, `provider_routing` green; no tool/adapter wire change in 007 commits | pass |
| No ordinary file above threshold without explicit exception | `orchestration_module_size_ratchet` covers every in-scope meta module over 1600 lines/80KB; `maintenance.md` table carries ceilings + next slices | pass |
| Dispatch has clear submodule boundaries | `dispatch/mod.rs` facade, `types.rs` owns job/config/output + capability partition, `execution.rs` owns bounded executor; `decomposed_dispatch_layout` guard | pass |
| Dependency parsing has single normalized contract | `dependency_parse/mod.rs` owns `parse_dependency_file`; per-ecosystem submodules; `decomposed_dependency_layout` guard | pass |
| Forge behavior has clear ownership | `forge_adapter.rs` owns execution + shared safety (base-URL validation, address classification, `ForgeReadBudget`, bounded reads); host split tracked as next slice; forge guards lock invariants | pass |
| Local-code intelligence has clear boundaries | `local/mod.rs` facade documents backend/inventory/cache/symbols/ignore/safe-open ownership + constraints; physical move under `local/` tracked as next slice; git/safe-open guards hold | pass |
| Evidence packaging no longer owns ranking | `evidence_bundle_owns_packaging_not_ranking`: bundle owns `build_evidence_bundle`, never `FetchCandidateBuilder`/`rank_and_select`; ranking owns builder + ordering, never bundle semantics | pass |
| Repeated validation uses shared owners | `canonical_helpers_have_single_owner`: 6 helpers in `canonical.rs`, domain tools call via `canonical::`, never duplicate | pass |
| Typed grouping not flattened | `suggested_fetch_group_helpers_stay_typed`: research/security keep typed `recommended_extract_mode_for_group` | pass |
| Shared workflow primitives consumed | `workflows_consume_shared_primitives`: suggested-fetch builders via `FetchCandidateBuilder`, adapters via `RetrievalAttemptSet` | pass |
| Dead/compat paths classified | `compatibility_dead_code_inventory`: 40 annotations in 16 allowed files, ceiling 45 | pass |
| Guards prevent reconcentration | Ratchet ceilings fail on growth; `modular_tool_and_adapter_layout` warns at 1600 for tools/adapter | pass |
| Full focused suites green | `forge_adapter`, `property_local_fs`, `property_local_fs_extended`, `provider_workstream_regression` pass (see §5) | pass |

## 3. Before/after file-size table (lines / bytes on candidate)

| Module | Before (plan observation or pre-slice) | After (candidate) | Ceiling | Disposition |
|---|---|---|---|---|
| `dispatch.rs` → `dispatch/` | 2379 single file | `mod` 18, `types` 179, `execution` 2204 (total 2401 with facade docs) | 100 / 400 / 2300 | Decomposed; executor loop next slice is deadlines/telemetry/merge extraction |
| `dependency_parse.rs` → `dependency_parse/` | 1683 single file (007 slice: 1737 split) | `mod` 784 + 17 ecosystem/status submodules (current total reflects later hardening ecosystems; each file under its ceiling) | 800 / 400 each | Decomposed; per-ecosystem ownership |
| `forge_adapter.rs` | ~97KB single file | 3012 lines / 100200 bytes | 3050 / 101000 | Ratchet + next slice (host-independent vs host-specific); safety invariants locked |
| `local_backend.rs` | ~97KB single file | 2634 / 97265 | 2700 / 100000 | Ratchet + next slice (move under `local/`) |
| `evidence_bundle.rs` | ~73KB | 2033 / 73378 | 2150 / 81920 | Ratchet; owns packaging + gaps, never ranking |
| `local_inventory_cache.rs` | ~60KB | 1953 / 61768 | 2000 / 81920 | Ratchet + next slice (move under `local/`) |
| `fetch_ranking.rs` | ~49KB | 1510 / 48730 | 1600 / 81920 | Ratchet; ranking owner |
| `local_inventory.rs` | ~50KB | 1464 / 49679 | 1600 / 81920 | Ordinary ceiling |
| `local_symbols.rs` | ~42KB | 1224 / 42326 | 1600 / 81920 | Ordinary ceiling |
| `security_search.rs` | large orchestration | 2166 / 84127 | 2300 / 88000 | Ratchet; native advisory + applicability pipeline |
| `provider_diagnostics.rs` | large | 2094 / 77984 | 2200 / 81920 | Ratchet |
| `research_workflow.rs` | 1749 unceilinged | 1749 / 65772 | 1900 / 81920 | Ceiling added in this sequence; typed workflow preserved |
| `adapter/tests.rs` | 2546 unceilinged | 2546 / 90773 | 2700 / 100000 | Fixtures-only ceiling added; production modules stay under ordinary ceilings |

Core (`config`, `provider`, `security`, `retrieval_status`) and fetch
(`client`, `cache`, `pdf`, `span`) files over 1600 lines are outside
007 scope (plan observations cover meta orchestration only) and are not
claimed as 007 evidence; they remain subject to ordinary discipline,
not the 007 ratchet.

## 4. Responsibility map (extracted modules)

- `dispatch/types.rs`: `DispatchJob`, `DispatchConfig`, `DispatchOutput`,
  capability partition (`PartiallySupported`/`Unsupported` →
  `SkippedCapabilityUnavailable`, partial roles preserved); never
  `dispatch_parallel`, never `rq_` role derivation.
- `dispatch/execution.rs`: bounded concurrent executor
  (`dispatch_parallel`); never role derivation from labels.
- `dependency_parse/mod.rs`: normalized `parse_dependency_file`
  dispatch, `DependencyParseReport` seam, basename extraction, Go
  vendor gating; never `parse_cargo_lock`/`parse_go_mod`/`parse_pom_xml`.
- `dependency_parse/<ecosystem>.rs`: one parser per ecosystem, one
  normalized output; shared path/size/root validation stays outside.
- `local/mod.rs` (facade): backend → `local_backend`, discovery →
  `local_inventory`, cache/runner → `local_inventory_cache`, parsing →
  `local_symbols`, ignore → `local_ignore`, opening → `safe_open`;
  single cache abstraction, bounded budgets, root containment, regex
  fallback.
- `evidence_bundle.rs`: deterministic handoff packaging (dedup,
  linking, caps, trust/provider summaries, gaps); never ranking math.
- `fetch_ranking.rs`: `FetchCandidateBuilder`, `rank_and_select`,
  scoring; never bundle packaging.
- `mcp/tools/canonical.rs`: `parse_repo_goal`,
  `parse_research_goal`, `parse_security_goal`,
  `resolve_repo_semantics`/`resolve_repo_sources`,
  `resolve_research_workflow`/`resolve_research_includes`,
  `resolve_security_workflow`/`resolve_security_includes`.

## 5. Removed duplicates and dead-code inventory

Consolidated helpers (one owner, callers via seam):

- 3 goal parsers + 3 workflow resolvers (plus source/include
  resolvers) moved from `repo_search.rs`/`research_search.rs`/
  `security_search.rs` into `canonical.rs`; domain files contain zero
  duplicate `fn parse_*_goal` definitions (guard-enforced).
- Suggested-fetch construction consolidated behind existing
  `FetchCandidateBuilder`; domain planners keep typed group helpers by
  design (over-deduplication forbidden by guard).
- Evidence/ranking split enforced statically; no new abstraction
  introduced.

Dead-code inventory (`compatibility_dead_code_inventory`):

- 40 `#[allow(dead_code)]` annotations across 16 allowed files
  (retrieval-state helpers, dispatch internals, 10 engine protocol
  fallbacks, local test helpers, startup platform policy), ceiling 45.
- Each retained path carries an exit condition class (required
  compatibility, test-only, feature-gated, removable) per the plan;
  new annotations outside the inventory fail CI.

## 6. Verification executed

Against candidate `85a44b7` (superset of `fbf8457` for production code;
M006 closure adds no production delta):

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test forge_adapter
cargo test --locked --all-features --test property_local_fs
cargo test --locked --all-features --test property_local_fs_extended
cargo test --locked --features mock --test provider_workstream_regression
cargo test --locked --all-features --test mcp_tool_contract --test mcp_schema_slimming --test mcp_2026_protocol --test mcp_projection
cargo test --locked --all-features
make docs-check
make bench-check
make check
```

Outcomes:

- `cargo fmt --check`: pass
- `clippy --all-targets --all-features -D warnings`: pass
- `static_guards`: 44 passed (includes 007 ratchet, dispatch/dependency layout, evidence/ranking, canonical single-owner, typed helpers, dead-code inventory)
- `forge_adapter`: 87 passed (safety invariants retained after non-split; all hosts share safety owner)
- `property_local_fs` + `property_local_fs_extended`: 57 passed (root containment, symlinks, traversal)
- `provider_workstream_regression` (mock): 7 passed
- Tool consolidation contracts: 14 + 16 + 13 + 14 passed
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make docs-check`: pass
- `make bench-check`: pass
- `make check` (fmt + clippy + no-default check + all-features tests + hygiene + packaging-check): pass
- hygiene + packaging-contract: ok

Ecosystem parser corpora (`security_applicability_*`,
`property_dependency_parse`, `dependency_fixtures`) remain green;
dependency decomposition introduced no parser behavior change (corpus
parity via pre-existing parser tests).

## 7. Invariant review

Ten tools, adapter ownership, capability separation, legacy
compatibility, and typed repo/research/security semantics preserved.
No provider parsing moved into workflow modules; no workflow policy
moved into engine adapters. No broad dependency or async-runtime
rewrite combined with this plan.

## 8. Failure and recovery review

Bounded I/O and execution guards unchanged and green
(`no_unbounded_forge_body_reads`, `all_forge_response_paths_bounded`,
`forge_has_aggregate_byte_budget_type`, `no_unbounded_git_output`,
`git_runner_drains_stdout_before_stderr_concurrently`,
`no_path_based_reads_in_safe_open`). Budget breaches degrade to
partial/fallback evidence, never uncontrolled failure.

## 9. Migration and compatibility review

No storage migration, no protocol change, no public request/response
change. Stable `crate::meta::dispatch::X` and
`crate::meta::dependency_parse` paths preserved via re-exports.
`local_*` paths preserved (facade, no import churn).

## 10. Security review

No new network, execution, or authorization surface. SSRF/credential/
redirect policy remains in one forge owner; per-host modules must not
duplicate it (guard-enforced by bounded-read and budget-type guards).
Local workspace constraints (no execution, bounded budgets, root
containment, structured-with-regex-fallback, single cache) preserved
in the facade and covered by local-FS property suites.

## 11. Documentation and operations

- `architecture/maintenance.md`: ownership table, overlap ratchets,
  file-size ceilings with next slices, guard index, verification
  commands.
- `architecture/meta.md`, `architecture/overview.md`,
  `skills/eggsearch-architecture/SKILL.md`: module maps reflect
  `dispatch/` and `dependency_parse/` decomposition plus `local/`
  facade.
- `docs/test-inventory.md` + `architecture/testing.md` +
  `skills/eggsearch-dev/SKILL.md`: suite counts in sync.
- No new top-level directory introduced against repository policy;
  `local/` facade reuses existing layout.

## 12. Residual findings

| Severity | Finding |
|---|---|
| Low, tracked next slice | `forge_adapter.rs` host-independent vs host-specific physical split not yet extracted; guarded by ratchet + forge safety guards. |
| Low, tracked next slice | `local_*` files not yet moved under `local/`; facade + ratchet + local-FS guards hold boundaries. |
| Low, tracked next slice | `dispatch/execution.rs` deadline/telemetry/merge extraction not yet split; ~1400 lines are inline fault-injection tests. |
| None high/medium | No behavior, contract, or safety regression identified. |

These slices require no new architecture decision and do not block
closure; they are tracked in the maintenance table with ceilings that
fail on regrowth.

## 13. Roadmap disposition

M007 moves to closed with this record as controlling evidence. With
M006 already closed in this sequence, the remaining workstream item is
M001-M005 closure records (implementation landed, records pending).
No new corrective plan is required; future maintenance follows the
ratchet guards.

## 14. Registry updates

Covered in the same closure commit: M007 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/007-status.md`;
subsystem remains active until M001-M005 closure records land.
