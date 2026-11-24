# M004 Repository-Read Consolidation — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/mcp-tool-surface-succession/004-repository-read-consolidation.md`

Roadmap: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` M004

Decision authority: ADR-0022 (accepted 2026-11-22 on this candidate), bounded by ADR-0001's adapter
ownership and ADR-0016's alias mechanism.

## Exact code candidate

Baseline `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`.
The implementation is the working tree on top of that baseline; no implementation commit exists yet
because the requesting instruction was "no commit unless requested". Every probe below ran against
that tree.

## Change

### Production

| File | Change |
|---|---|
| `src/mcp/tool_contract.rs` | `repo_fetch` contract removed; every `related_tools`/`next_tools` pointer retargeted to `fetch`; `repo_search` description no longer points at the alias; registry test 10 → 9 |
| `src/mcp/advertisement.rs` | `repo_fetch` removed from full, research and code presets and from the specialist instruction line; preset sizes 9/6/6/3; pinned full instructions regenerated |
| `src/core/workflow.rs` | `STABLE_TOOL_NAMES` without `repo_fetch` |
| `src/sdk.rs` | `ToolName::ALL` is nine entries without `RepoFetch`; `FromStr` keeps `repo_fetch` as an early compatibility branch, so the Python/SDK execute path still resolves it |
| `src/integrations/common.rs` | recommended-tool list without `repo_fetch` (6 tools) |
| `src/meta/suggested_fetches.rs` | `preferred_tool` is `fetch` at all five emission sites; the executable `batch_item`/`structured_repo_fetch` already carried the locator |
| `src/meta/recipe_catalog.rs` | `repo_search_next_actions` emits a `fetch` action with `items: [{type: "repo", ...}]`; two workflow recipe steps and two canonical step-tool lists retargeted |
| `src/mcp/server.rs` | `repo_fetch` removed from the compact description and compact-field dispatch; the `#[tool(name = "repo_fetch")]` handler is untouched and still routed |
| `src/core/repo_search.rs` | doc comment naming the preferred tool |

No new tool, argument, schema property, provider, configuration key, or dependency. Adapter
ownership, validation, sanitization, SSRF, bounds, trust, stable IDs, and repository-read behaviour
are unchanged.

### Tests, fixtures, packaging, bindings

`mcp_tool_contract`, `static_guards`, `docs_tool_names`, `mcp_schema_slimming`, `mcp_tools`,
`tool_surface_evaluation` (including `tests/fixtures/tool_surface/cases.json`),
`tool_surface_live`, `tool_inventory_parity`, the two `repo_search` corpus scenarios,
`packaging/release-smoke.sh`/`.ps1`, and `bindings/python/tests/test_native.py` now describe nine
advertised names plus one more alias. The evaluation keyword table's repo-file bonuses were
retargeted from `repo_fetch` to `fetch`, because that harness scores tool-name and description
tokens only.

### Documents and decision records

ADR-0022 moved to accepted, with narrow supersession pointers appended to ADR-0001, ADR-0016, and
ADR-0020 (their decisions are otherwise not rewritten). `plans/000-long-term-specification.md` §1,
§2, and §7 now state nine and name ADR-0022. `AGENTS.md`, `README.md`, `docs/tool-matrix.md`,
`docs/features.md`, `docs/agent-workflows.md`, `docs/codegg-integration.md`, `docs/config.md`,
`docs/python-library.md`, `docs/provider-setup.md`, `docs/safety.md`, `docs/threat-model.md`,
`docs/retrieval-quality-codegg-handoff.md`, `docs/test-inventory.md`, `architecture/{mcp,overview,
core,fetch,meta,maintenance,local-workspace,hardening,engines,integrations,testing,codegg-contract,
config}.md`, both skills, `bindings/python/PYTHON_README.md`, and `CHANGELOG.md` were updated.
`docs/retrieval-quality-codegg-handoff.md` no longer pins a tool-surface fingerprint.

## Live acceptance on the exact tree

Debug build, MCP stdio, default configuration.

```text
ADVERTISED 9 ['build_evidence_bundle', 'fetch', 'knowledge_brief', 'provider_status',
              'repo_map', 'repo_search', 'research_search', 'security_search', 'web_search']
HAS_REPO_FETCH False
```

Same locator (`rust-lang/log` `README.md`, `max_chars: 400`), one process:

| Call | Result |
|---|---|
| `repo_fetch` direct | ok, 14,645 bytes of JSON, no error |
| `fetch` with one `type: "repo"` item | ok, 15,075 bytes of JSON |
| first 60 characters of `text` | identical |
| first `lines[0].text` | identical |

The batch response is the same `RepoFetchResponse` object plus `focus` and the batch envelope; the
envelope difference is preserved rather than normalized, as ADR-0022 requires.

Follow-up rewriting is covered by `mcp_tool_contract` (`repo_search` suggests `fetch`) and by the
evaluation corpus, whose repo-fetch cases now expect `fetch`. A `repo_search` suggestion therefore
names a tool the caller can see and carries an executable repo item.

## Deviations from the plan, and why

- **The blast radius was larger than the plan's table.** `src/meta/recipe_catalog.rs` emitted
  `repo_fetch` as a recipe step, as two next actions, and in two canonical step-tool lists; the plan
  named only `suggested_fetches.rs`. All were updated because a recipe that names an unadvertised
  tool would have those steps silently dropped by the preset filter at the response seam.
- **`provider_status` keeps its `repo_fetch` capability keys.** They describe a callable tool's
  policy and block-expansion capabilities, which are still true. They are not a discovery list.
- **The evaluation keyword table changed.** It scores name and description tokens, so leaving the
  repo-file bonuses on the retired name would have made a known-file query rank `repo_search` above
  `fetch`. The bonuses moved with the capability.
- **The compact/full 50% size ratio became 25%.** Consequence of M002, recorded in that closure
  record, not of this milestone.

## Python binding status

The `bindings/python` package imports a built `eggsearch._native` extension, which is not present in
this environment, so the Python suite was not executed here. Static verification:

- `EXPECTED_TOOLS` is the nine-name set.
- `AsyncEggsearch.repo_fetch` and `RepoFetchRequest`/`RepoFetchResponse` remain, so the convenience
  method still calls `execute("repo_fetch", ...)`.
- Discovery derives from `EggsearchClient::tool_names()`, which reads `ALL_CONTRACTS`; the Rust
  parity suite proves that projection is nine.
- The M004 stop condition (a binding that cannot keep `repo_fetch` callable without advertising it)
  did not trigger: the alias resolves through the `FromStr` compatibility branch.

## Residual risk

- **Two envelopes remain.** Alias callers receive `RepoFetchResponse`; `fetch` callers receive a
  `results[]` item. ADR-0022 preserves that difference deliberately, so a migrated consumer must
  handle the new shape.
- **`repo_fetch` still appears in diagnostic output** (`provider_status` capability keys) and in
  module paths and internal type names. That is accurate, and a tool-inventory audit must read those
  as capability/diagnostic facts, not as advertised names.
- **`tools/call repo_fetch` remains unbounded by discovery.** Any caller that knows the name can
  still invoke it; this is the same compatibility posture ADR-0016 established for the other two
  aliases.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_inventory_parity   # 4 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tools                # 29 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract         # 22 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_tool_names          # 3 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards             # 67 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_evaluation   # 5 passed
RUSTUP_TOOLCHAIN=1.89.0 make check                                                         # exit 0
RUSTUP_TOOLCHAIN=1.89.0 make docs-check                                                    # exit 0
python3 packaging/check-planning-consistency.py                                           # ok
```
