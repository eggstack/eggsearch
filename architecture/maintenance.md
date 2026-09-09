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
| Fetch execution | `src/fetch/` (client, limits, cache, origin, browser) | Search ranking, evidence roles |
| Release/deploy surface | `src/platform.rs`, `src/update.rs`, `src/startup.rs`, `packaging/` | Search/fetch policy |

Module size warning (enforced by `static_guards.rs`): tool and adapter
modules warn over 1,600 lines — split further rather than growing a god
module.

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
