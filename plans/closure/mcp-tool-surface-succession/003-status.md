# M003 One Inventory, Every Projection — Closure Evidence

Status: closed

Source plan: `plans/archive/implementation/mcp-tool-surface-succession/003-tool-inventory-parity.md`

Roadmap: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` M003

Decision authority: ADR-0001 (fixed inventory), ADR-0016 §5–§6 (alias boundary). No new ADR.

## Exact code candidate

Baseline `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`.
The implementation is the working tree on top of that baseline; no implementation commit exists yet.

## Change

1. **New `tests/tool_inventory_parity.rs`** (4 tests) asserting, in both directions:
   - `ALL_CONTRACTS` names == `tool_names()` == `ToolName::ALL` == `STABLE_TOOL_NAMES` ==
     `EggsearchClient::tool_names()` == the union of the four preset tables;
   - every preset is a subset of the canonical names, alphabetically ordered, duplicate-free;
   - the compatibility alias set is exactly one named constant, each alias is absent from every
     discovery projection and from `is_known_tool`/`is_stable_tool`, each still parses through
     `ToolName::from_str`, and each still appears in `routed_tool_names()`;
   - the documented preset table in `docs/tool-matrix.md` equals `advertisement::tools_for` for
     all four presets, including the advertised count;
   - the documented canonical list in `architecture/codegg-contract.md` §1 equals `tool_names()`.
   The count is always derived from `ALL_CONTRACTS`; no literal ten (or nine) is baked into the
   suite, which is what let M004 change the inventory without rewriting it.
2. **Real drifts corrected.**
   - `architecture/config.md` claimed the presets advertised 7/8/4 tools. The code and its tests
     said 7/7/3. The copied counts are replaced by a pointer to the single table.
   - `architecture/overview.md` drew a "10 tools" box naming `web_fetch` and `batch_fetch` and
     omitting `fetch` and `knowledge_brief`. The box now points at `docs/tool-matrix.md`.
3. **`tests/static_guards.rs`** now asserts the alias names rather than a count, and that each
   alias stays out of `ToolName::ALL`, `STABLE_TOOL_NAMES`, and every preset.
4. **`tests/docs_tool_names.rs`** keeps its presence and release-smoke checks; preset-table and
   canonical-list parsing delegates to the new suite.

## Failing-before / passing-after

All three injected drifts were introduced, observed, and reverted.

**Documented preset row gains an unadvertised name**

```text
thread 'documented_preset_table_matches_advertisement' panicked at tests/tool_inventory_parity.rs:275:9:
assertion `left == right` failed: documented Research advertised count differs from tools_for
test result: FAILED. 0 passed; 1 failed; ... 3 filtered out
```

**Documented full-preset count changes**

```text
thread 'documented_preset_table_matches_advertisement' panicked at tests/tool_inventory_parity.rs:275:9:
assertion `left == right` failed: documented Full advertised count differs from tools_for
  left: 10
 right: 9
test result: FAILED. 0 passed; 1 failed; ... 3 filtered out
```

**Documented canonical list gains an alias**

```text
thread 'documented_codegg_canonical_list_matches_contract_names' panicked at tests/tool_inventory_parity.rs:310:5:
assertion `left == right` failed: CodeGG canonical tool list differs from tool_names()
  left: ["build_evidence_bundle", "fetch", "web_fetch", "knowledge_brief", ...]
test result: FAILED. 0 passed; 1 failed; ... 3 filtered out
```

After reverting each injection the suite is 4 passed, 0 failed.

## Deviations from the plan, and why

- **Documented preset table compared order-insensitively.** The plan asked for equality against
  `tools_for`. The code tables are asserted alphabetically in their own right; the documented table
  groups names logically for a human reader. Comparing sorted sets keeps the assertion strict about
  membership and count without forcing a documentation reorder that carries no meaning.
- **No generated documentation block.** The plan's stop condition allowed replacing the table with a
  generated block if Markdown parsing proved brittle. The parser fails closed instead: it panics on a
  missing heading, a changed row count, or an unrecognised count word, so a documentation rewrite
  cannot silently skip the check.
- **The executing worker terminated on an upstream provider timeout** after writing the suite and
  the guard changes and before producing its report. The parent completed the two document fixes and
  captured all three drift demonstrations above, so the evidence is first-hand.

## Residual risk

- The suite depends on two documented shapes: the `## Advertisement presets` heading plus its first
  table, and the `## 1. Stable Tool Surface` table. A structural rewrite must keep those anchors or
  update the suite in the same change; it fails loudly rather than passing empty.
- `packaging/release-smoke.*` inventories are checked by `tests/docs_tool_names.rs`, not by this
  suite. That is deliberate: the smoke scripts test the shipped binary, and the parity suite tests
  the library projections.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_inventory_parity  # 4 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_tool_names         # 3 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract        # 22 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test static_guards            # 67 passed
RUSTUP_TOOLCHAIN=1.89.0 make check                                                        # exit 0
```
