# M042 — Fetch Item Shape Diagnostics

Status: ready
Date: 2026-11-20
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M029, fetch tool argument contract)
Baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`
Decision authority: existing contracts — accepted ADR-0001 (ten-tool surface) and accepted
ADR-0016 (unified fetch tool and legacy aliases). No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
`plans/subsystems/agentic-retrieval-integrity-roadmap.md` M029, M036, M039;
original plan `plans/implementation/agentic-retrieval-integrity/029-unified-fetch-tool-and-legacy-aliases.md`;
original closure `plans/closure/agentic-retrieval-integrity/029-status.md`;
`architecture/mcp.md`; `docs/tool-matrix.md`.

## Objective

Make a malformed `fetch` item produce an actionable error instead of a serde implementation
detail.

A `fetch` item is a tagged union: `FetchItem` is
`#[serde(tag = "type", rename_all = "snake_case")]` (`src/mcp/tools/fetch.rs:17`), so
`type` is required and the accepted forms are `{"type": "web", ...}` and
`{"type": "repo", ...}`. The code already knows how to describe those forms, but only for an
item that carries a tag. An item with no `type` at all bypasses that check and surfaces the
raw deserialization failure.

## Evidence

Measured on 2026-11-20 against baseline `16b4c2888206caeb6654ffebaa039862fc143712`, calling
`fetch` through the MCP server over stdio.

1. `fetch` with `{"items": [{"url": "https://example.com/"}]}` — a web item missing only its
   `type` tag — returned the MCP error
   `failed to deserialize parameters: data did not match any variant of untagged enum Repr`.
2. The caller learns neither the accepted shapes nor which field was wrong. The same request
   with `{"type": "web", "url": "..."}` succeeded and returned the document.
3. `validate_fetch_item_shape` (`src/mcp/tools/fetch.rs:176`) already emits an actionable
   message for a `repo` item missing `owner`, `repo`, or `path` (line 186 onwards). It
   branches on `object.get("type")` and therefore only runs for an item that already has a
   tag.
4. The failing error text names an internal enum, `Repr`, which is a private implementation
   type in `FetchItems` (`src/mcp/tools/fetch.rs:140`). It is not part of the tool contract and
   is not actionable for a caller or an agent.

## Why the earlier verification missed this

M029 unified the fetch tool and its legacy aliases and established the argument contract. Its
acceptance covered the successful shapes and the two legacy aliases; no criterion asserted the
error text for a malformed item. The existing shape validation was written to describe a missing
*field* on a tagged item, and the missing-*tag* case was never routed into it, so the gap was
invisible to the suite.

## Scope

In scope:

- a missing `type` tag on a `fetch` item producing an actionable error that names the accepted
  shapes;
- an unknown `type` value producing an actionable error that names the accepted values;
- regression coverage for the single-item and array forms.

Out of scope:

- changing the accepted item shapes, the tag name, the schema, or the tool contract;
- changing the existing `repo` and `web` field-level messages;
- the `batch_fetch` alias payload, which shares the item contract only insofar as M029 defines
  it — if the alias surfaces the same text through the same validator, that follows from this
  milestone rather than being separate work;
- any new tool, argument, schema property, configuration key, or dependency.

## Invariants

- The accepted item shapes are unchanged. A valid `web` or `repo` item is accepted exactly as
  before, and the advertised schema is unchanged.
- The existing actionable message for a tagged item missing a required field is unchanged.
- The error stays a validation error on the same tool; it does not become a partial success, a
  silent skip, or a different status code.
- The message names the accepted shapes and never leaks a private Rust type name.
- No new tool, argument, schema property, or dependency.

## Expected production changes

- `src/mcp/tools/fetch.rs`: extend `validate_fetch_item_shape` with a missing-tag branch and
  an unknown-tag branch, reusing the existing accepted-shape text.
- `tests/mcp_tools.rs` and the fetch suite that owns the tool contract: regression coverage.
- `docs/tool-matrix.md` (the `fetch` accepted-shape note) and `docs/test-inventory.md`.

## Compatibility and migration

- No tool, argument, schema, or configuration change.
- Behaviour change: a malformed item reports a different, more actionable message. A caller
  matching on the exact old string is not a supported contract, and the old string named an
  internal type.
- No index, storage, or dependency change. No migration.

## Dependencies

- Hard: M029 (closed) — this is a corrective pass to the fetch argument contract M029
  established.
- Interface: accepted ADR-0016 for the unified fetch tool and its aliases.
- Soft: none.

## Work packages

1. **Paper first.** Update `docs/tool-matrix.md` to state the accepted item shapes for
   `fetch` in one place, including the required `type` tag. This lands before code.
2. **Missing tag.** When a fetch item is an object with no `type` key, report the accepted
   shapes — `{"type":"web","url":"<url>"}` and
   `{"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}` — and say the tag is
   missing. Reuse the existing message text so the two forms read as one contract.
3. **Unknown tag.** When `type` is present but not one of the accepted values, name the
   accepted values. Keep the existing field-level checks for a known tag untouched.
4. **Both forms.** Apply the check to a single item and to every item of an array, keeping the
   existing per-item index prefix.
5. **Tests.** Extend the fetch tool suite with:
   - a missing `type` tag returns an actionable error naming both accepted shapes and does not
     mention an internal type name;
   - an unknown `type` value returns an actionable error naming the accepted values;
   - a valid `web` item and a valid `repo` item are unaffected;
   - a tagged `repo` item missing a required field keeps its existing message;
   - the array form reports the offending index.
6. **Documentation.** `docs/tool-matrix.md`, `docs/test-inventory.md`, `CHANGELOG.md`.

## Verification

```bash
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test static_guards
make check
make docs-check
```

A live acceptance probe is required: the same malformed `fetch` call before and after, with
both response bodies recorded.

## Acceptance criteria

- A `fetch` item with no `type` tag returns an actionable error naming both accepted shapes.
- A `fetch` item with an unknown `type` value returns an actionable error naming the accepted
  values.
- No fetch error message names a private Rust type.
- A valid `web` item and a valid `repo` item behave exactly as before.
- The existing tagged-item field-level message is unchanged.
- The array form reports the offending index.
- No new tool, argument, schema property, configuration key, or dependency.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- making the message actionable would require changing the accepted item shapes or the schema —
  the contract is fixed and this milestone only improves the error;
- the check cannot be applied to the array form without changing the existing per-item prefix
  behaviour — record the divergence rather than special-casing one form;
- the same defect surfaces through the `batch_fetch` alias with a different payload contract —
  record it and scope it as separate work rather than widening this milestone;
- the fix would require a new error code or status the MCP contract does not already define.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/042-status.md` with the exact candidate SHA, the
M029 closure reference, a requirement-to-evidence matrix, the before/after malformed-call
responses, the suite and broad-gate results, and residual risk.
