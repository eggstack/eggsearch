# Maintenance, Ownership, and Extension Guidance

**Purpose:** keep the post-consolidation repository from re-concentrating
into new monoliths. This is the contributor contract for adding tools,
providers, workflows, parsers, and tests.

The crate is application-first (Policy A, see `src/lib.rs`): the stable
contract is MCP tools plus CLI. The Rust module tree is an implementation
detail for the binary, integration tests, and fuzz harnesses.

---

## Ownership boundaries

| Area | Owner module | Must not own |
|------|--------------|--------------|
| Tool validation + response shape | `src/mcp/tools/<tool>.rs` + shared `common.rs` | Engine dispatch, RRF, workflow policy |
| Transport lifecycle | `src/mcp/server.rs`, `src/mcp/http.rs` | Domain orchestration (`build_*_plan`, `dispatch_subqueries`, `aggregate_rrf`, evidence analysis) — call tool seams instead (enforced by `static_guards.rs`) |
| Search orchestration | `src/meta/adapter/` (invocation, advisory, status, web/repo/research/security execution, normalization, builders) | Engine-specific HTTP parsing |
| Upstream parsing | `src/meta/engines/<provider>.rs` via `EngineSearchRequest` | Workflow policy, role assignment, coverage |
| Shared workflow mechanics | `src/meta/workflow.rs` (`PlannedLane`, `WorkflowExecution`, `RetrievalAttemptSet`, `FetchCandidateSet`) + `fetch_ranking.rs` (`FetchCandidateBuilder`) | Typed domain semantics (repo/research/security keep their own planners, grouping, suggested-fetch builders on top of the shared primitives) |
| Bounded dispatch | `src/meta/dispatch/` (`types` for job/config/output types + capability partition, `execution` for the bounded concurrent executor) | Engine-specific parsing, workflow policy, role derivation from labels |
| Dependency parsing | `src/meta/dependency_parse/` (`mod` owns the normalized `parse_dependency_file` dispatch; `cargo`/`npm`/`go`/`python`/`ruby`/`composer`/`maven`/`dotnet`/`containers`/`github_actions` own one ecosystem each) | Path/size/root validation, applicability policy |
| Local workspace | `src/meta/local/` facade + `local_backend.rs` (search orchestration), `local_inventory.rs` (git discovery/identity), `local_inventory_cache.rs` (cache + git runner), `local_symbols.rs` (structured parsing), `local_ignore.rs`, `safe_open.rs` | Cross-owner logic (see subsystem table below); single cache abstraction only |
| Forge access | `src/meta/forge_adapter.rs` (execution + shared safety owner: base-URL validation, address classification, bounded reads via `read_bounded_body`/`ForgeReadBudget`, redirect rejection) | Duplicated SSRF/credential/redirect policy in per-host code |
| Evidence packaging | `src/meta/evidence_bundle.rs` (`build_evidence_bundle`: dedup, linking, caps, trust/provider summaries, gaps) | Ranking math and candidate ordering (owned by `fetch_ranking.rs`) |
| Fetch ranking | `src/meta/fetch_ranking.rs` (`FetchCandidateBuilder`, `rank_and_select`, scoring) | Domain source semantics and bundle packaging |
| Fetch execution | `src/fetch/` (client, limits, cache, origin, browser) | Search ranking, evidence roles |
| Release/deploy surface | `src/platform.rs`, `src/update.rs`, `src/startup.rs`, `packaging/` | Search/fetch policy |

Module size ratchet (enforced by `static_guards.rs`
`orchestration_module_size_ratchet`): ordinary source files must stay
under 1,600 lines and 80 KB. Files above either threshold require an
explicit exception below; ceilings prevent silent regrowth. Decomposed
directories (`dispatch/`, `dependency_parse/`) are gated per file so no
submodule can reconcentrate complexity.

---

## Extension rules

- New MCP tools get dedicated modules under `src/mcp/tools/` with shared
  validation in `common.rs`. Register in `src/mcp/server.rs` (`#[tool]`
  attrs); `static_guards.rs` requires exactly ten stable tools unless the
  tool-matrix, docs contract tests, and CodeGG integration docs are updated
  in the same change.
- Provider adapters do not own workflow policy. New engines implement
  `SearchEngine::search(&EngineSearchRequest)` with defaulted advisory
  methods; unsupported capabilities stay explicit so dispatch can emit
  capability-skip attempts instead of silent omissions.
- Domain workflows use shared execution primitives but retain typed domain
  semantics. `repo`/`research`/`security` planners, grouping, and
  suggested-fetch builders must construct candidates via shared
  `FetchCandidateBuilder` and record attempts via shared
  `RetrievalAttemptSet` (enforced by `static_guards.rs`); do not flatten
  them into one generic workflow type.
- New providers require an evidence-class/capability justification: declare
  the 24-flag `ProviderCapabilities`, add the ID to `KNOWN_PROVIDER_IDS`,
  document native versus local enforcement in `docs/provider-setup.md` and
  `AGENTS.md`, and extend `tests/provider_capability_contract.rs`.
- New local parser backends implement the bounded `SymbolBackend` seam
  (`src/meta/local_backend.rs` + `src/meta/local_symbols.rs`); regex
  remains the fallback. Budgets breach to partial/regex evidence, never
  failure. No workspace code execution.
- New integration tests belong to behavior-oriented suites (`mcp_tools`,
  `web_search`/`web_fetch` integration, `provider_routing`,
  `provider_probe_conformance`, `repo`/`research`/`security` workflow,
  `evidence_contract`, plus the `*_contract`/`*_retrieval` regression
  suites). Do not introduce new `phase<N>_*` names; preserve fixture
  provenance in file-level doc comments instead.
- Factual inventories must be code-derived where practical:
  `docs_tool_names.rs` derives from `src/mcp/server.rs`,
  `docs_provider_inventory.rs` derives from `KNOWN_PROVIDER_IDS`,
  `provider_capability_contract.rs` locks native-enforcement semantics.
  Do not generate narrative documentation from code.

---

## 007 decomposition record (maintenance closure slice)

Before/after sizes (total lines include inline tests;-behavior is
unchanged, verified by the existing all-features, property,
fault-injection, forge-safety, and parser suites):

| Module | Before | After | Responsibility split |
|--------|--------|-------|----------------------|
| `dispatch.rs` (2,379 lines / 96 KB) | one file | `dispatch/mod.rs` (18) + `dispatch/types.rs` (179) + `dispatch/execution.rs` (2,204, of which ~1,400 are pre-existing inline tests; non-test executor ~750) | Types + capability partition vs bounded concurrent execution mechanics; deadline/telemetry/merge stay inside `execution.rs` as the next split candidate |
| `dependency_parse.rs` (1,683 / 60 KB) | one file | `dependency_parse/mod.rs` (669, incl. dispatch + shared XML helpers + pre-existing corpus tests) + `cargo` (139) + `npm` (177) + `go` (94) + `python` (226) + `ruby` (55) + `composer` (39) + `maven` (179) + `dotnet` (65) + `containers` (57) + `github_actions` (47) | One normalized `parse_dependency_file` contract; each ecosystem parser owns one file and emits the shared `DependencyFinding` record |
| Local workspace | four large files, no facade | `local/mod.rs` facade (27) documenting the subsystem map; `local_backend`/`local_inventory`/`local_inventory_cache`/`local_symbols` implementation files unchanged | Facade establishes the `local/` subsystem boundary; physical moves under it are the next slice (paths stay stable via re-exports) |
| `forge_adapter.rs` (2,920 / 97 KB) | one file, unchanged | Unchanged file; shared safety ownership documented, invariants locked by guards | Host-independent safety (base-URL validation, address classification, bounded reads, redirect rejection) stays in one owner; host-specific GitHub/GitLab/Gitea/Forgejo execution split is the next slice |
| `evidence_bundle.rs` / `fetch_ranking.rs` | overlap risk | Unchanged files; ownership ratchet added | Bundle owns deterministic handoff packaging, never ranking math; ranking owns scoring/ordering, never bundle semantics (enforced by `evidence_bundle_owns_packaging_not_ranking`) |

Explicit exceptions (above 1,600 lines or 80 KB, ratcheted to prevent
growth; each has a tracked next slice or a standing justification):

- `forge_adapter.rs` (2,920 / 97 KB) — next slice is the
  host-independent vs host-specific split; safety invariants locked by
  `no_unbounded_forge_body_reads`, `all_forge_response_paths_bounded`,
  `no_object_sha_in_commit_urls`, `forge_has_aggregate_byte_budget_type`.
- `local_backend.rs` (2,595 / 97 KB) — next slice moves backend logic
  under `local/`; constraints (no execution, bounded budgets, root
  containment, structured-then-regex) unchanged.
- `evidence_bundle.rs` (2,033 / 73 KB) — lines over threshold, bytes
  under; owns packaging + gap analysis, never ranking.
- `local_inventory_cache.rs` (1,906 / 60 KB) — next slice moves cache +
  git runner under `local/`; bounded-execution invariants locked by
  `no_unbounded_git_output` and
  `git_runner_drains_stdout_before_stderr_concurrently`.
- `security_search.rs` (2,180 / 85 KB) — security orchestration with
  native advisory + applicability pipeline; per-provider outcomes locked
  by advisory guards.
- `provider_diagnostics.rs` (2,094 / 78 KB) — provider health + routing;
  capability-skip semantics locked by dispatch guards.
- `dispatch/execution.rs` (2,204 / 91 KB) — newly extracted executor;
  inline fault-injection tests account for ~1,400 lines; non-test
  executor is ~750 lines. Next slice splits deadlines/telemetry/merge
  out of the executor loop.

Overlap ratchet (enforced by `static_guards.rs`):

- `workflows_consume_shared_primitives` — domain suggested-fetch
  builders must use `FetchCandidateBuilder`; domain adapters must record
  via `RetrievalAttemptSet`.
- `evidence_bundle_owns_packaging_not_ranking` — bundle never owns
  ranking math; ranking never owns bundle semantics.
- `canonical_helpers_have_single_owner` — `canonical.rs` owns
  goal/workflow/source resolution; domain tools call it, never duplicate
  it.
- `suggested_fetch_group_helpers_stay_typed` — the
  `recommended_extract_mode_for_group` look-alikes in research/security
  are intentionally typed per domain (`ResearchResultGroupKind` vs
  `SecurityResultGroupKind`) and must stay that way; over-deduplication
  across distinct group taxonomies is forbidden.
- `compatibility_dead_code_inventory` — `#[allow(dead_code)]` is capped
  (45) and restricted to the documented file inventory (engines protocol
  fallbacks, startup platform policy, retrieval-state helpers, dispatch
  internals, local test helpers). New annotations outside the inventory
  fail; each retained path carries its exit condition (required
  compatibility, test-only, feature-gated, or removable).

Canonical validation audit (007-G): `parse_repo_goal`,
`parse_research_goal`, `parse_security_goal`, `resolve_repo_semantics`,
`resolve_research_workflow`, and `resolve_security_workflow` live in
`src/mcp/tools/canonical.rs` (Plan 002 seam). Domain tools resolve
through them; the guard fails closed on duplicates.

## Hygiene and docs

- `packaging/check-repo-hygiene.sh` (`make hygiene`, part of `make check`)
  rejects tracked transcripts (`typescript`), ANSI dumps at the repo root,
  tracked build outputs, oversized unexpected root blobs, and editor temp
  files. Test/corpus fixtures under `tests/` and `docs/` are legitimate
  and never matched.
- Keep `packaging/release-targets.txt`, the release workflow, installers,
  and installation docs synchronized; `make packaging-check` catches drift.
- Keep `docs/test-inventory.md`, `architecture/testing.md`, and
  `skills/eggsearch-dev/SKILL.md` synchronized when adding or renaming
  suites. `CHANGELOG.md` entries are historical and are never rewritten.

---

## Verification for maintenance changes

```bash
make check
make docs-check
make bench-check
```

`make check` is fmt + clippy + no-default-features compile check +
all-features tests + hygiene + packaging contract. Routine work stays
network-free; live probes and native-forge smokes run only on explicit
opt-in targets.

---

[← Back to Overview](overview.md)
