# MCP Tool-Surface Succession Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md` §1, §2, §7
- `plans/001-terminology-and-domain-model.md` §1, §3
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`
- `plans/adrs/ADR-0016-unified-fetch-discovery-and-legacy-alias-compatibility.md`
- `plans/adrs/ADR-0020-cutoff-aware-knowledge-brief-tool.md` (raised the advertised count from nine to ten)
- `plans/adrs/ADR-0022-nine-tool-surface-after-repo-fetch-retirement.md` (proposed)

Supersedes no roadmap. The tool-surface-consolidation roadmap
(`plans/subsystems/tool-surface-consolidation-roadmap.md`) is closed at M001–M007 and is
not reopened; this roadmap owns the surface after those milestone closure records.

## 1. Purpose and ownership boundary

Owns the advertised MCP inventory once the consolidation workstream is closed: which names
are canonical, how that one inventory is projected into the contract registry, core workflow
sanitization, advertisement presets, the SDK facade, release smoke scripts, and the
documented tables. Also owns advertised input-schema economy where one tool's schema is
disproportionate to its capability.

Does not own retrieval semantics, provider selection, fetch safety policy, ranking, or the
domain planners. Those remain with search capability, agentic retrieval integrity, and the
domain workflow owners. This roadmap may change what a caller sees; it may not change what a
call returns for the same valid input.

## 2. Work classification

### Invariants

- The canonical inventory is exactly ten tools today. It becomes nine only after ADR-0022 is
  accepted and M004 lands; no other milestone may change the count.
- Adapter ownership, the `core <- meta <- mcp <- commands` dependency flow, sanitization,
  bounds, trust markers, and content-derived stable IDs are unchanged.
- Compatibility aliases stay callable, unadvertised, and outside `next_actions` and SDK
  discovery (ADR-0016 §5).
- Advertised schema bytes never grow as a side effect of restructuring.

### Capabilities

- One documented surface that a contributor can change without hunting for a second copy.
- Any surface change moves every projection in the same commit, enforced by tests.

### Infrastructure

- One source per advertised argument list and per goal vocabulary.
- A bounded schema projection for the evidence bundle that does not narrow the runtime types.
- A parity guard across the inventory's projections.

### Polish

- Measured schema-byte reduction and removal of stale prose. Not a capability claim.

## 3. Non-goals

- Collapsing the repo, research, and security planners into one generic workflow tool. The
  canonical specification forbids flattening them, and their execution paths are not
  redundant.
- Removing the `web_fetch` or `batch_fetch` dispatch path. ADR-0016 promises callable
  compatibility, and `web_fetch` is the shared web executor rather than a duplicate
  implementation (see §4).
- New tools, arguments, schema properties, providers, configuration keys, or dependencies.
- A general documentation-generation framework. A single authoritative table plus tests is
  the target; generated prose is not.
- Changing search, fetch, browser, PDF, cache, or trust behaviour.

## 4. Baseline state

Measured 2026-11-22 on `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` by querying `tools/list` over
`mcp stdio`. This is the pre-milestone baseline; §13 records the post-milestone measurements:

| Fact | Value |
|---|---|
| Advertised tools | 10 |
| Callable names | 12 (`web_fetch` and `batch_fetch` are routed but unadvertised) |
| Preset sizes | full 10, research 7, code 7, minimal 3 |
| Total advertised input-schema bytes | 64,850 |
| `build_evidence_bundle` input schema | 39,543 B (61.0% of the total) |
| of which `$defs` | 37,516 B across 39 definitions |
| Top `$defs` | `RankReason` 3,572, `CodeEvidence` 2,814, `SourceMetadata` 2,666, `SourceKind` 1,887 |

Overlap findings:

- One true capability duplicate: a `fetch` repo item and `repo_fetch` build the same
  `core::repo_fetch::RepoFetchRequest` (`src/mcp/tools/batch_fetch.rs:450` and `:472`). Every
  `RepoFetchArgs` field exists on `FetchRepoItem`, plus `focus`; only the response envelope
  differs.
- `web_fetch` is not a second implementation. Its `run_web_fetch_inner` is the shared web
  executor: `src/mcp/tools/batch_fetch_item.rs:73` builds `WebFetchArgs` and `:100` calls
  `run_web_fetch_with_prepared_client`. `batch_fetch` is a fan-out layer over it.
- The four search tools converge on an argument grammar (4/4 `query`, `freshness`,
  `max_results`, `response_detail`; 3/4 `goal`, `max_per_group`) and on one result envelope,
  but their planners and provider sets differ. Grammar convergence is not redundant
  capability.
- The same inventory is projected six ways: `src/mcp/tool_contract.rs`,
  `src/core/workflow.rs` `STABLE_TOOL_NAMES`, `src/mcp/advertisement.rs` presets,
  `src/sdk.rs` `ToolName`, `packaging/release-smoke.*` expected sets, and the documented
  tables.

Existing guards: `tests/docs_tool_names.rs` (presence, not list equality),
`tests/mcp_tool_contract.rs` (ten contracts), `tests/static_guards.rs:1207` (ten canonical
plus two aliases), `tests/mcp_schema_slimming.rs` (byte budgets).

Known drift found during the audit and not yet fixed:

- `architecture/config.md:214` states the code preset advertises 8 tools and minimal 4; the
  code and its tests say 7 and 3.
- `architecture/overview.md:25` lists `web_fetch` and `batch_fetch` among the "10 tools"
  where `fetch` and `knowledge_brief` belong.

Both are assigned to M003.

## 5. Target architecture

One inventory with named projections, each test-enforced. A contributor changing the surface
edits the projection that owns the fact and the parity test names every other place that must
agree. Advertised schemas stay bounded per tool; the evidence bundle advertises a passthrough
shape instead of inlining output type graphs. Compatibility aliases remain a finite,
name-based dispatch set.

## 6. Dependency graph

```text
M001 search-schema single-sourcing (independent)
M002 evidence-bundle input projection (independent)
M003 inventory parity + documented truth (independent)
    |
    +--> M004 repository-read consolidation (soft: M003 makes the surface change complete)
              |
              `--> ADR-0022 acceptance (hard gate for the count change)
```

M001, M002, and M003 are dependency-ready now and are ordered by risk, not value.

## 7. Milestones

### M001 — Search-schema single-sourcing

Class: infrastructure. Objective: one maintained source per advertised search argument list
and per goal vocabulary, with the full and compact profiles advertising the same accepted
values. Implementation plan:
`plans/implementation/mcp-tool-surface-succession/001-search-schema-single-sourcing.md`.
Exit: a drift in either direction fails a test; no accepted value and no advertised byte
budget changes.

### M002 — Evidence-bundle input projection

Class: infrastructure + polish. Objective: cut the `build_evidence_bundle` input schema to a
bounded passthrough projection without narrowing the runtime types or changing bundle
identity. Implementation plan:
`plans/implementation/mcp-tool-surface-succession/002-evidence-bundle-input-projection.md`.
Exit: measured reduction with unchanged acceptance fixtures and a lowered byte ceiling.

### M003 — One inventory, every projection

Class: infrastructure. Objective: make the inventory projections agree by test and remove
duplicated counts from prose. Implementation plan:
`plans/implementation/mcp-tool-surface-succession/003-tool-inventory-parity.md`.
Exit: injected drift in any projection fails; the two known stale statements are corrected or
replaced by pointers.

### M004 — Repository-read consolidation

Class: capability + compatibility. Objective: advertise `fetch` as the only repository read
operation and keep `repo_fetch` as a callable compatibility alias. Implementation plan:
`plans/implementation/mcp-tool-surface-succession/004-repository-read-consolidation.md`.
Dependencies: hard ADR-0022 acceptance; soft M003.
Exit: nine advertised tools, `repo_fetch` still callable with its current request and
response, every projection and document updated in the same candidate.

## 8. Cross-cutting requirements

Storage: no migration. Protocol: the MCP tool list changes only in M004, and only after
ADR-0022; aliases keep their request and response contracts. Security: adapter ownership,
sanitization, SSRF, bounds, and trust are unchanged; no new outbound path. Documentation:
`docs/tool-matrix.md` is the single authoritative table, other documents point at it.
Compatibility: Python and SDK convenience calls keep working, including
`repo_fetch` after M004.

## 9. Verification strategy

`RUSTUP_TOOLCHAIN=1.89.0 make check` and `make docs-check` per milestone, plus focused
`mcp_schema_slimming`, `mcp_tool_contract`, `docs_tool_names`, `tool_surface_evaluation`, and
the new parity suite. Schema work records before/after `tools/list` measurements for the full
and compact profiles in the closure record. Surface work records a `tools/list` name diff and
a successful alias call on the exact candidate.

## 10. Risks and decision points

- ADR-0022 is the only open decision. Until it is accepted, the inventory stays at ten and
  M004 is blocked.
- The `build_evidence_bundle` nested input types feed `source_id` through
  `metadata.source_kind` (`src/meta/evidence_bundle.rs:116`). M002 must therefore be a schema
  projection, not a type narrowing; a type change would risk stable-ID semantics.
- M001 can become churn for a small byte win. Its stop condition requires a removed
  duplicate, not a smaller struct.
- M004 touches the Python facade and release smoke scripts. If the binding cannot retain
  `repo_fetch` as a callable non-discovered operation, M004 stops for a maintainer decision.

## 11. Completion definition

Closed when M001–M004 each have accepted closure evidence on an exact candidate: one
inventory whose projections are test-enforced, a bounded evidence-bundle schema, and either a
nine-tool advertised surface with a working `repo_fetch` alias (if ADR-0022 is accepted) or a
recorded decision to keep ten.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/implementation/mcp-tool-surface-succession/001-search-schema-single-sourcing.md` | `plans/closure/mcp-tool-surface-succession/001-status.md` | None |
| M002 | closed | `plans/implementation/mcp-tool-surface-succession/002-evidence-bundle-input-projection.md` | `plans/closure/mcp-tool-surface-succession/002-status.md` | None |
| M003 | closed | `plans/implementation/mcp-tool-surface-succession/003-tool-inventory-parity.md` | `plans/closure/mcp-tool-surface-succession/003-status.md` | None |
| M004 | closed | `plans/implementation/mcp-tool-surface-succession/004-repository-read-consolidation.md` | `plans/closure/mcp-tool-surface-succession/004-status.md` | ADR-0022 accepted 2026-11-22 |

## 13. Post-milestone measurements

Measured on the implementation tree after M004 (baseline `b2abc59`):

| Fact | Baseline | Final |
|---|---|---|
| Advertised tools | 10 | 9 |
| Callable names | 12 | 12 (nine canonical plus `web_fetch`, `batch_fetch`, `repo_fetch`) |
| Preset sizes | full 10, research 7, code 7, minimal 3 | full 9, research 6, code 6, minimal 3 |
| Full `tools/list` serialized | 72,388 | 31,521 |
| Full per-tool sum | 72,368 | 31,511 |
| Compact `tools/list` serialized | 19,739 | 18,686 |
| `build_evidence_bundle` definition | 40,152 | 2,578 |
| `build_evidence_bundle` `$defs` | 39 | 1 |

Every projection named in §4 is now covered by `tests/tool_inventory_parity.rs`.
