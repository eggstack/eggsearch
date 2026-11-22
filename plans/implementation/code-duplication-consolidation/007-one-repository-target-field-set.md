# M007 — One Repository-Target Field Set

Status: closed (ADR-0023 accepted 2026-11-22)
Closure: `plans/closure/code-duplication-consolidation/007-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: compatibility + infrastructure
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: `plans/adrs/ADR-0023-one-repository-target-field-set.md` (accepted
2026-11-22). Acceptance is recorded in the ADR, this plan, and the registry in the same change.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
ADR-0016 §5–§6 (alias contract); ADR-0022 (nine-tool surface);
`architecture/fetch.md` § Repository targets; `docs/tool-matrix.md`.

Dependencies: hard ADR-0023 acceptance. Soft M001 (`Default` on the argument structs makes the
composition ergonomic in tests).

## Objective

Define the sixteen common repository-locator fields once in `RepoTarget`, compose that locator
into the three remaining envelopes, retire the duplicate `FetchRepoItem`, and build the execution
request through one conversion, without changing the JSON contract.

## Current evidence

Measured 2026-11-22 on the baseline:

| Shape | Location | Fields | Extras beyond the locator |
|---|---|---|---|
| `RepoFetchArgs` | `src/mcp/tools/repo_fetch.rs:24` | 20 | `host`, `timeout_ms`, `response_detail`, `test_fetch_url` |
| `FetchRepoItem` | `src/mcp/tools/fetch.rs:74` | 20 | identical to `RepoFetchArgs` |
| `BatchFetchItem::Repo` | `src/core/batch_fetch.rs:67` | 20 | `host`, `focus`, `focus_max_chunks`, `focus_max_chars` |
| `RepoFetchRequest` | `src/core/repo_fetch.rs:124` | 18 | `host`, `timeout_ms` |

The sixteen common fields are exactly the intersection; the arithmetic reproduces 20/20/20/18.
`host` stays per shape by ADR-0023 decision 2: the wire shapes need `Option<String>` for alias
diagnostics, and the execution request needs `Option<CodeHost>`.

The conversion sites that currently re-map fields by hand:
`src/mcp/tools/fetch.rs:241` (`into_batch_parts`), `src/mcp/tools/repo_fetch.rs` (args to
request), and `src/mcp/tools/batch_fetch.rs` (item to request). The prior workstream's M001 and
M004 closure records name this field set as the main source of blast radius.

The advertised schema guard already asserts the property names:
`tests/mcp_schema_slimming.rs:247` requires every advertised `fetch` repo-item property to be
present, so a composition that drops or renames a property fails immediately.

## Invariants that cannot regress

- Every JSON property name, accepted value, description, and default visible to a caller is
  unchanged. A caller's request that parses today parses after.
- `repo_fetch` returns the same response for the same request; it stays callable and
  unadvertised (ADR-0016 §5, ADR-0022).
- `fetch`, `batch_fetch`, and `repo_fetch` keep their current validation messages, including
  the unknown-host message that names the accepted aliases.
- The advertised `fetch` schema stays inside its existing per-tool budget
  (`tests/mcp_schema_slimming.rs`) and the full-profile total does not grow.
- `src/core` keeps no dependency on `meta`, `mcp`, or `commands`.
- No new dependency, tool, argument, or configuration key.

## Work packages

1. **Gate.** Confirm ADR-0023 is accepted and recorded. If it is still `proposed`, stop before
   any edit and report.
2. **Record the baseline.** Capture, for the full profile: the `fetch` definition bytes, its
   input-schema bytes, its `$defs` count, and the full `tools/list` serialized total; and for
   `repo_fetch`, one alias call's response on a fixed local fixture. Store the table for the
   closure record.
3. **Add `core::repo_fetch::RepoTarget`** with the sixteen fields, their current documentation,
  `#[serde(default, skip_serializing_if = "Option::is_none")]` attributes, `JsonSchema`, and
  `Default`. Do not include `host`.
4. **Compose the envelopes.**
   - `RepoFetchArgs`: `host`, `#[serde(flatten)] target`, `timeout_ms`, `response_detail`,
     `test_fetch_url`.
   - `BatchFetchItem::Repo`: `host`, `#[serde(flatten)] target`, the focus trio.
   - `RepoFetchRequest`: `host: Option<CodeHost>`, `#[serde(flatten)] target`, `timeout_ms`.
5. **Retire `FetchRepoItem`.** Make `fetch`'s `Repo` variant hold `RepoFetchArgs` and delete the
   duplicate type. Update `FetchItem::into_batch_parts` (`src/mcp/tools/fetch.rs:241`) to the one
   conversion.
6. **One conversion.** Implement `From<RepoTarget> for RepoFetchRequest` (plus host and timeout
   supplied by the caller) and route the alias handler, the batch item, and the fetch item
   through it. Delete the hand-written field-by-field mappings.
7. **Parity test.** Add a test that serializes a fully populated `RepoTarget` into each envelope
   and asserts the key set equals `RepoTarget` keys plus that envelope's extras, and that a JSON
   object with every field deserializes into each envelope without a lossy drop.
8. **Re-pin and measure.** Update the schema pins and budgets that the measurement touches:
   `tests/mcp_tool_contract.rs` (fingerprint/bytes), `tests/mcp_schema_slimming.rs`,
   `tests/tool_surface_live.rs` (`BASELINE_TOTAL_BYTES`, `BASELINE_EST_TOKENS`),
   `tests/mcp_2026_protocol.rs`, `tests/mcp_http.rs`, and `tests/schema_identity_registry.rs` if it
   pins the type name. Recompute from the new build; never adjust a budget upward without the
   ADR's fallback decision.
9. **Docs.** Update `docs/tool-matrix.md` and the repository-target section of
   `architecture/fetch.md` only if a description or property list changed; if none changed, say
   so in the closure record instead of editing prose.

## Expected production changes

| File | Change |
|---|---|
| `src/core/repo_fetch.rs` | `RepoTarget` and one conversion |
| `src/core/batch_fetch.rs` | `Repo` variant composes the locator |
| `src/mcp/tools/repo_fetch.rs` | `RepoFetchArgs` composes the locator |
| `src/mcp/tools/fetch.rs` | `FetchRepoItem` removed; variant holds `RepoFetchArgs` |
| `src/mcp/tools/batch_fetch.rs` | Conversion routed through the one `From` |
| Schema pin files | Recomputed, not guessed |

## Non-goals

- Moving `host` into the locator or teaching `CodeHost` serde aliases (ADR-0023 decision 2).
- Unifying the four search argument structs with their core request types.
- Changing validation rules, accepted hosts, path rules, or response fields.
- Changing the advertised tool list, presets, or alias dispatch.
- Refactoring the repository fetch execution path in `meta::forge_adapter`.

## Compatibility and migration

The JSON contract is unchanged; the Rust type layout changes. A test or downstream Rust consumer
that constructs the envelopes field by field must switch to the composed locator; the repository
is a single crate, so the compiler finds every site. Python and SDK consumers see no difference.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test schema_identity_registry
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test batch_fetch_retrieval
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tools
# opt-in live probe; requires EGGSEARCH_EVAL_MODEL and network, never a gate
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_live -- --ignored
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Parity evidence: the fixed-fixture alias call before and after must return the same `text`,
`lines`, and stable identifiers; the round-trip test proves no field is dropped.

## Acceptance and stop conditions

- One locator field list, one conversion, three envelopes, and no `FetchRepoItem`.
- Every advertised property name is still present, the `fetch` budget holds, and the full total
  does not grow.
- The alias returns the same response on the fixed fixture.
- **Stop:** if flattening pushes `fetch` over its per-tool budget or changes a description in a
  way that cannot be restored, stop and apply ADR-0023's macro fallback, or return the ADR for a
  maintainer decision. Do not raise a byte budget to make the refactor fit.

## Required closure evidence

The ADR acceptance record, the before/after schema table, the updated pin values, the parity test
and round-trip test output, the alias before/after fixture, and the full gate result.
