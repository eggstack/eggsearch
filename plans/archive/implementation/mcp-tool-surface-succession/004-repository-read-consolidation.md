# M004 — Repository-Read Consolidation

Status: closed
Decision: ADR-0022 was accepted on 2026-11-22 and implemented on this candidate.
Date: 2026-11-22
Subsystem: MCP tool-surface succession
Milestone class: capability + compatibility
Baseline: `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`
Decision authority: ADR-0022 (proposed), bounded by ADR-0001's adapter ownership and
ADR-0016's alias mechanism.

References: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` §4, §7;
`plans/adrs/ADR-0022-nine-tool-surface-after-repo-fetch-retirement.md`;
`plans/000-long-term-specification.md` §1, §2, §7; `architecture/codegg-contract.md` §1;
`docs/tool-matrix.md`.

Dependencies: hard ADR-0022 acceptance; soft M003 (the parity guard makes the multi-surface
change complete).

## Objective

Advertise `fetch` as the only repository read operation and keep `repo_fetch` callable as a
finite compatibility alias, so one capability has one advertised name.

## Current evidence

- A `fetch` repo item and `repo_fetch` reach the same core request:
  `src/mcp/tools/batch_fetch.rs:450` and `:472` construct
  `core::repo_fetch::RepoFetchRequest`. `FetchRepoItem` carries every `RepoFetchArgs` field
  except the request-level `timeout_ms` and `response_detail`, and adds `focus`.
- Generated follow-ups already know how to express the replacement:
  `src/meta/suggested_fetches.rs:216` sets `preferred_tool: Some("repo_fetch")`, and
  `core::fetch_locator::structured_repo_fetch_to_batch_item` converts the locator into a
  `fetch` repo item.
- The SDK already has the exact compatibility shape required: `ToolName` keeps
  `WebFetch`/`BatchFetch` variants and `FromStr` branches for them, while
  `ToolName::ALL` excludes them (`src/sdk.rs:15–90`). `repo_fetch` should follow that shape,
  not invent a new one.
- The surface is broad: 326 occurrences of `repo_fetch` across 88 files at the baseline,
  including `bindings/python/python/eggsearch/__init__.py:118`,
  `bindings/python/tests/test_native.py`, `tests/fixtures/tool_surface/cases.json`, corpus
  scenarios, release smoke scripts, `AGENTS.md`, `docs/tool-matrix.md`, and
  `architecture/codegg-contract.md`.

## Invariants that cannot regress

- `repo_fetch` remains callable with its current request and response contract, through MCP
  `tools/call` and the SDK/Python execute path.
- Repository-read behaviour is unchanged: workspace and forge validation, ref resolution,
  line and symbol selection, PDF handling, sanitization, trust markers, and bounds.
- `fetch` repo items return equivalent content for the same locator.
- No new tool, argument, provider, configuration key, or dependency is added.
- Adapter ownership and the dependency flow are unchanged; no handler calls an engine
  directly.

## Work packages

1. **Contract and advertisement.** Remove the `repo_fetch` entry from `ALL_CONTRACTS` and from
   the full, research, and code preset tables. `EggsearchServer::new` filters routed tools by
   `is_known_tool`, so the handler becomes unadvertised without moving code. Confirm that
   `hidden_tools` and the preset-closure tests still describe the surface correctly.
2. **Follow-up rewriting.** Replace `preferred_tool: Some("repo_fetch")` with `fetch` and make
   the emitted action carry the repo item produced by
   `structured_repo_fetch_to_batch_item`. Update `core::workflow::STABLE_TOOL_NAMES` and
   confirm `is_stable_tool` and `is_known_tool` agree. No `next_actions` entry may name
   `repo_fetch`, and no suggestion may require the caller to reconstruct a locator.
3. **SDK and Python.** Move `RepoFetch` to the compatibility shape: keep the enum variant and
   the `FromStr` branch, remove it from `ToolName::ALL`, and keep the Python `repo_fetch`
   convenience method working through `execute("repo_fetch", ...)`. Update
   `bindings/python/tests/test_native.py` expected inventories to nine, while keeping a direct
   `repo_fetch` parity case.
4. **Inventories and fixtures.** Update `tests/docs_tool_names.rs`, `tests/mcp_tool_contract.rs`,
   `tests/static_guards.rs`, `tests/tool_surface_evaluation.rs`,
   `tests/fixtures/tool_surface/*`, affected corpus scenarios, and the expected sets in
   `packaging/release-smoke.sh` and `.ps1`.
5. **Documents and specification.** Amend `plans/000-long-term-specification.md` §1, §2, and §7
   to state nine advertised tools and the `repo_fetch` alias, following the ADR-0016 precedent.
   Update `AGENTS.md`, `docs/tool-matrix.md`, `docs/features.md`,
   `docs/agent-workflows.md`, `docs/codegg-integration.md`, `docs/test-inventory.md`,
   `architecture/{mcp,codegg-contract,overview,core,maintenance}.md`, the skills, and
   `CHANGELOG.md`.
6. **ADR bookkeeping.** Mark ADR-0022 accepted and append the narrow supersession pointers to
   ADR-0001, ADR-0016, and ADR-0020, following the practice already visible in ADR-0016. Do not
   rewrite their decisions.

## Expected production changes

| File | Change |
|---|---|
| `src/mcp/tool_contract.rs` | Remove the `repo_fetch` contract |
| `src/mcp/advertisement.rs` | Remove it from three presets |
| `src/core/workflow.rs` | `STABLE_TOOL_NAMES` without `repo_fetch` |
| `src/meta/suggested_fetches.rs` | `preferred_tool = fetch`, locator preserved |
| `src/sdk.rs` | `RepoFetch` out of `ALL`, kept in `FromStr` and dispatch |
| tests, fixtures, packaging smoke | Nine-name inventories plus an alias call |
| `bindings/python` | Expected inventories; convenience method retained |
| docs, spec, skills, changelog | Nine-tool statement and alias description |

## Non-goals

- No change to `fetch`'s schema, budgets, or behaviour.
- No change to `repo_map`, `repo_search`, or any search tool.
- No removal of `repo_fetch` dispatch, and no deprecation warning.
- No response-envelope normalization between `fetch` results and `RepoFetchResponse`.
- No new tool to "replace" the ninth slot.

## Compatibility and migration

Alias-compatible for callers: an existing `repo_fetch` request returns the same
`RepoFetchResponse`. Discovery-based consumers see nine names and must use `fetch` with a repo
item for new work. The Python convenience method and SDK execute path keep working. This is a
discovery and documented-surface change, not a behavioural one.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_inventory_parity
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tools
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_tool_names
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib sdk
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
./packaging/check-contract.sh
```

Plus the Python suite in `bindings/python` and an exact-candidate `tools/list` diff showing
nine names and no `repo_fetch`.

Live acceptance on the release build, recorded in the closure record:

- a direct `repo_fetch` call for a known file still returns the same content;
- a `fetch` repo item for the same locator returns equivalent content;
- a `repo_search` response's follow-up names `fetch` and is executable as emitted.

## Acceptance and stop conditions

- `tools/list` advertises nine canonical names; `repo_fetch` is callable and absent from all
  discovery projections.
- Every projection and document updated on the same candidate.
- **Stop:** if the Python binding cannot keep `repo_fetch` callable without also advertising it,
  stop and record the binding constraint for a maintainer decision rather than breaking the
  published API.

## Required closure evidence

The closure record carries the exact-candidate `tools/list` diff, the alias call and `fetch`
equivalence evidence, the follow-up rewriting example, the projection-by-projection update
list, the Python and packaging results, the gates, and residual risk (a second envelope remains
for alias callers).
