# M001 — Search-Schema Single-Sourcing

Status: closed
Date: 2026-11-22
Subsystem: MCP tool-surface succession
Milestone class: infrastructure
Baseline: `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`
Decision authority: ADR-0001 (adapter ownership), ADR-0016 (fetch unification). No new ADR.

References: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` §4, §7;
`plans/000-long-term-specification.md` §2; `architecture/mcp.md` (§ Canonical contracts,
§ Response projection); `docs/tool-matrix.md`; `tests/mcp_schema_slimming.rs`.

Dependencies: none.

## Objective

Give each advertised search argument list and each accepted `goal` vocabulary exactly one
maintained source, and make the full and compact schema profiles advertise the same accepted
values.

## Current evidence

Measured at the baseline with `tools/list`:

- The four search tools share `query`, `freshness`, `max_results`, and `response_detail`
  (4/4) and `goal` and `max_per_group` (3/4). Their planners and provider sets differ, so the
  shared grammar is real and intentional; only its declaration is duplicated.
- `goal` is `Option<String>` in all four tools. The accepted values live in
  `src/mcp/tools/canonical.rs` as `REPO_GOAL_VALUES`, `RESEARCH_GOAL_VALUES`, and
  `SECURITY_GOAL_VALUES`, consumed by `parse_repo_goal`, `parse_research_goal`, and
  `parse_security_goal`.
- The full schema advertises no `goal` enum. The compact profile injects one at runtime for
  `research_search` and `security_search` only (`src/mcp/server.rs`,
  `compact_workflow_schema`), using a fourth, hand-written list in `server.rs`. The two
  profiles therefore advertise different contracts for the same field.
- The compact profile keeps a second per-tool property allow-list in
  `src/mcp/server.rs::compact_schema`. Nothing asserts that a listed name still exists in the
  generated schema, so a rename silently hides a field.
- Approximately 27 `#[schemars(skip)]` legacy properties remain across the four tools. They
  are accepted on the wire and hidden on purpose; this milestone does not remove them.

## Invariants that cannot regress

- No tool, argument, or accepted value changes. The `parse_*` functions and their constants
  remain the runtime authority.
- An advertised `goal` enum must never accept a value the runtime rejects, and must never
  reject a value the runtime accepts.
- Compact still hides exactly the fields it hides today. Any change to that set is an
  explicit, reviewed edit, not a side effect.
- Advertised schema bytes do not grow. `tests/mcp_schema_slimming.rs` budgets stay at or
  below their current values.
- `tools/list` stays deterministic; a changed content fingerprint is expected and must be
  recomputed, never pinned blind.

## Work packages

1. **One vocabulary per tool.** Annotate `goal` (and `research_domain` where an enum is
   advertised) so the enum is generated from the same constant the `parse_*` function uses.
   Delete the runtime enum injection in `compact_workflow_schema` for `goal` and
   `research_domain` once the full schema carries them. One function must not be able to
   advertise a different set in the two profiles.
2. **One compact allow-list per tool.** Move each tool's compact field allow-list next to its
   argument type (an associated const or a module constant) and have
   `server.rs::compact_schema` consume it. `server.rs` keeps the projection mechanics, not the
   per-tool data.
3. **Drift guards.** Add tests that fail when (a) a compact allow-list names a property the
   generated full schema does not contain, (b) a `goal` constant and its advertized enum
   disagree, and (c) the full and compact profiles advertise different accepted values for the
   same field.
4. **Shared grammar, only if it removes a duplicate.** Optionally factor `freshness`,
   `max_results`, `max_per_group`, `response_detail`, and `timeout_ms` into one shared struct
   flattened into the four argument types. Do this only if it deletes a maintained list and
   leaves property order and schema shape unchanged. Otherwise leave the structs separate and
   record why in the closure. The required outcome is item 1–3, not flattening.
5. **Documentation.** State in `architecture/mcp.md` that `goal` enums are advertised in both
   profiles and that compact allow-lists live with their argument types. No new table.

## Expected production changes

| File | Change |
|---|---|
| `src/mcp/tools/canonical.rs` | Enum-bearing constants or schema helpers next to the existing vocabularies |
| `src/mcp/tools/{web,repo,research,security}_search.rs` | `goal`/`research_domain` schema annotation; compact allow-list constants |
| `src/mcp/server.rs` | `compact_schema` reads per-tool constants; enum injection removed |
| `tests/mcp_schema_slimming.rs` | Drift assertions; unchanged or lower budgets |
| `architecture/mcp.md` | Both-profile vocabulary statement |

## Non-goals

- No new tool, argument, schema property, provider, or dependency.
- No change to ranking, planner selection, provider routing, or response projection.
- No removal of hidden legacy properties.
- No byte-reduction claim. Any reduction is measured and reported, not promised.

## Compatibility and migration

Wire-compatible. A client that sends a currently accepted `goal` keeps working. A client that
relied on compact's narrower enum now sees the same full accepted set in both profiles, which
is the intended correction; no documented value is removed.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test docs_tool_names
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_evaluation
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib mcp::tools::canonical
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib mcp::advertisement
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Record a before/after `tools/list` property dump for the four search tools in both profiles,
with byte counts.

## Acceptance and stop conditions

- One allow-list per tool; one vocabulary per tool; both profiles agree by test.
- A deliberately broken allow-list entry and a deliberately broken `goal` enum each fail a
  test.
- No advertised byte budget grows.
- **Stop:** if flattening (WP4) changes property order or schema shape without deleting a
  maintained duplicate, drop it and note the decision. Do not chase byte counts.

## Required closure evidence

The closure record carries the before/after property and byte tables for both profiles, the
list of deleted duplicates, the drift tests with a failing-before/passing-after demonstration,
the gates, and residual risk (hidden legacy properties unchanged; no provider behaviour
touched).
