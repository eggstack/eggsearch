# M003 — One Inventory, Every Projection

Status: closed
Date: 2026-11-22
Subsystem: MCP tool-surface succession
Milestone class: infrastructure
Baseline: `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`
Decision authority: ADR-0001 (fixed inventory), ADR-0016 §5–§6 (alias boundary). No new ADR.

References: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` §4, §7;
`plans/000-long-term-specification.md` §2; `architecture/codegg-contract.md` §1;
`architecture/mcp.md`; `docs/tool-matrix.md`; `tests/docs_tool_names.rs`;
`tests/static_guards.rs`.

Dependencies: none. M004 depends on this milestone as a soft dependency.

## Objective

Make every projection of the canonical tool inventory agree by test, and remove hand-copied
counts and lists from prose so the surface has one authoritative table and five enforced
projections.

## Current evidence

The inventory is projected six ways, all currently maintained by hand:

| Projection | Location |
|---|---|
| Contract registry | `src/mcp/tool_contract.rs::ALL_CONTRACTS` |
| Core follow-up gate | `src/core/workflow.rs::STABLE_TOOL_NAMES` / `is_stable_tool` |
| Advertisement presets | `src/mcp/advertisement.rs::FULL_TOOLS`, `RESEARCH_TOOLS`, `CODE_TOOLS`, `MINIMAL_TOOLS` |
| SDK discovery | `src/sdk.rs::ToolName::ALL` |
| Release smoke | `packaging/release-smoke.sh`, `packaging/release-smoke.ps1` |
| Documentation | `docs/tool-matrix.md`, `architecture/codegg-contract.md`, `architecture/overview.md`, `architecture/config.md` |

Existing guards cover only part of this: `tests/docs_tool_names.rs` asserts that each canonical
name appears somewhere and that release smoke matches `#[tool(name = ...)]` in
`server.rs`; `tests/mcp_tool_contract.rs` asserts ten contracts; `tests/static_guards.rs:1207`
asserts ten canonical tools plus two compatibility aliases; `tests/mcp_schema_slimming.rs`
holds byte budgets. None asserts that the projections agree with each other, and none parses a
documented preset table.

Two live drifts prove the gap:

- `architecture/config.md:214` says the code preset advertises 8 tools and minimal 4. The code
  and `advertisement.rs` tests say 7 and 3.
- `architecture/overview.md:25` lists `web_fetch` and `batch_fetch` among the "10 tools" where
  `fetch` and `knowledge_brief` belong.

The alias boundary is also under-asserted. `static_guards.rs` checks a count of two aliases,
not their names, and nothing asserts that the alias set stays out of `tools/list`,
`next_actions`, `ToolName::ALL`, and the presets.

## Invariants that cannot regress

- No advertised name, argument, accepted value, preset membership, or response field changes.
- Dispatch stays finite and name-based. Aliases remain callable; this milestone does not
  promote or retire any of them.
- `next_actions` keep their current sanitization behaviour: unknown or hidden targets are
  dropped, never rewritten.
- Documentation stays truthful: a pointer replaces a copied list, never the other way round.

## Work packages

1. **Parity suite.** Add `tests/tool_inventory_parity.rs` asserting, in both directions:
   - `ALL_CONTRACTS` names equal `ToolName::ALL` names, `STABLE_TOOL_NAMES`,
     `EggsearchClient::tool_names()`, and the union of the preset tables;
   - each preset table is a subset of the canonical names and is alphabetically ordered;
   - the compatibility alias set is exactly `{web_fetch, batch_fetch}`, each is absent from all
     four discovery projections, and each still parses and dispatches;
   - `is_stable_tool` and `is_known_tool` accept exactly the same canonical names.
2. **Documented table.** Parse the preset table in `docs/tool-matrix.md` and assert it equals
   `advertisement::tools_for(preset)` for all four presets, including the advertised count.
   Parse the canonical name list in `architecture/codegg-contract.md` §1 against
   `tool_names()`.
3. **Remove duplicated counts.** Replace the parenthetical preset sizes at
   `architecture/config.md:214` and the hard-coded ten-name box at
   `architecture/overview.md:25` with a pointer to the single table. Keep the surrounding
   explanation; delete only the copy that can drift.
4. **Alias boundary.** Strengthen `tests/static_guards.rs` to assert the alias names, not just
   their count, and that aliases stay out of presets, `next_actions`, and SDK discovery.
5. **Record the executor fact.** Add one sentence to `architecture/mcp.md` that `web_fetch` is
   the shared web executor used by `batch_fetch`/`fetch` web items
   (`src/mcp/tools/batch_fetch_item.rs:73`, `:100`), so a future overlap audit does not read it
   as a duplicate implementation.

## Expected production changes

| File | Change |
|---|---|
| `tests/tool_inventory_parity.rs` | New parity and alias-boundary suite |
| `tests/docs_tool_names.rs` | Preset-table assertions, or delegation to the new suite |
| `tests/static_guards.rs` | Alias names instead of a count |
| `architecture/config.md`, `architecture/overview.md` | Remove copied counts, add pointers |
| `architecture/mcp.md` | Executor-ownership sentence |

No production Rust code changes. If a parity assertion fails on the current tree, the fix is to
correct the document or the stale constant, not to widen the test.

## Non-goals

- No tool addition, removal, rename, or preset change.
- No schema change.
- No generated documentation; a pointer is sufficient.
- No change to alias dispatch or to ADR-0016's compatibility promise.

## Compatibility and migration

None. This milestone is documentation and test only, plus correcting statements that are
already false.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_inventory_parity
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_tool_names
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Failing-before evidence: temporarily change one documented preset count, add `repo_fetch` to a
preset, and add an alias to `ToolName::ALL`; each must fail a distinct assertion. Record the
three failures in the closure record.

## Acceptance and stop conditions

- All six projections agree by test.
- The two known drifts are corrected, and no new copied count replaces them.
- The alias set is asserted by name and by absence from discovery.
- **Stop:** if parsing a Markdown table proves brittle across formatting, replace the table
  with a generated block or a stable fenced list rather than writing a permissive parser that
  cannot fail.

## Required closure evidence

The closure record carries the before/after for both drifted documents, the three injected-drift
failures, the parity suite output, the gates, and a statement that no production behaviour
changed.
