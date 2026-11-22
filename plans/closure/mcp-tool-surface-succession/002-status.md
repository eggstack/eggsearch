# M002 Evidence-Bundle Input Projection — Closure Evidence

Status: closed

Source plan: `plans/implementation/mcp-tool-surface-succession/002-evidence-bundle-input-projection.md`

Roadmap: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` M002

Decision authority: ADR-0001 (adapter ownership). No new ADR.

## Exact code candidate

Baseline `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`.
The implementation is the working tree on top of that baseline; no implementation commit exists yet.
Every measurement below ran against that tree.

## Problem

`build_evidence_bundle` advertised 39,543 B of input schema, 61% of the whole advertised surface.
37,516 B of that was `$defs` across 39 definitions pulled in transitively by
`EvidenceSourceInput` and `EvidenceFetchInput` — output-side type graphs that a caller is
supposed to paste in, not author field by field.

## Change

1. `src/mcp/server.rs`: `compact_evidence_bundle_schema` became
   `project_evidence_bundle_schema`, applied by `apply_evidence_bundle_projection` to the full
   profile as well as inside the compact projection. It replaces the `sources` and `fetches` item
   schemas with bounded passthrough objects that keep the useful top-level names and set
   `additionalProperties: true`, and reduces `$defs` to `ResponseDetail`.
2. `src/mcp/tool_contract.rs` and `src/mcp/server.rs`: the tool description now tells a caller to
   pass prior output verbatim and states that nested metadata is accepted but not re-advertised.
3. `src/mcp/tools/evidence_bundle.rs`: the `sources` and `fetches` field docs state the same.
4. `tests/mcp_schema_slimming.rs`: new `evidence_bundle_input_schema_stays_bounded` ceiling, and
   the full-profile and total-definition ceilings lowered by the measured reduction.
5. `tests/evidence_contract.rs`: new identity snapshot over a request carrying nested
   `metadata` and a structured `locator`.
6. `docs/tool-matrix.md`, `architecture/mcp.md`: passthrough contract stated.

## Measured before and after

| Metric | Before | After | Delta |
|---|---|---|---|
| `build_evidence_bundle` definition bytes | 40,152 | 2,578 | −37,574 (−93.6%) |
| its input-schema bytes | 39,540 | 1,898 | −37,642 |
| its `$defs` entries | 39 | 1 | −38 |
| full `tools/list` serialized | 72,379 | 34,805 | −37,574 |
| full per-tool sum | 72,368 | 34,794 | −37,574 |
| compact `tools/list` serialized | 19,871 | 19,871 | 0 |

Compact is unchanged because the compact profile already applied the same projection; the change
makes the full profile advertise what compact proved was sufficient.

Ceilings set from measurement, not guessed: per-tool definition ≤ 3,000 B; full serialized < 40,000;
per-tool total ≤ 40,000. Compact's existing 22,540 ceiling is unchanged.

## Identity snapshot

`evidence_bundle_identity_is_stable_under_bounded_input_schema` builds from a JSON request that
carries `metadata.source_kind`, a structured `locator`, a line range, and `text`, and pins:

| Field | Value |
|---|---|
| `bundle_id` | `bundle_27becfbe6b86541c` |
| `source_id` | `src_673bbf9ba38b7620` |
| `fetch_id` | `fetch_6609ffa1b98e12f7` |
| `trust_summary.external_untrusted_count` | 1 |

The values were captured on the pre-change tree and are unchanged after the projection, which is the
point: nested metadata still reaches `compute_source_id`.

## Deviations from the plan, and why

- **Mechanism.** The plan proposed six per-field `#[schemars(schema_with)]` annotations. The
  existing `compact_evidence_bundle_schema` already produced exactly the required bounded shape,
  was already exercised by the compact profile, and removes all 38 transitive definitions rather
  than six. Reusing it is smaller, proven, and keeps one projection rather than two. Runtime types
  are untouched, which was the plan's actual constraint.
- **No field needed to stay advertised.** The plan's stop condition (a projected field needing
  schema-level validation to keep a stable ID or reject a malformed locator) did not trigger: the
  live alias/fetch calls below show nested metadata and locators still parse and still drive the
  same IDs.

## Live acceptance on the exact tree

Debug build, MCP stdio, default configuration:

```text
tools/list            -> 9 advertised names
repo_fetch (alias)    -> ok, 14,645 bytes of JSON
fetch repo item       -> ok, 15,075 bytes
alias text head       == fetch response text head   (identical 60 chars)
alias first line      == fetch response first line  (identical)
```

## Budget consequence recorded

`compact_profile_advertises_all_tools_with_smaller_stable_contract` previously required compact to
be at least 50% smaller than full. After this milestone full is 34,805 B and compact 19,871 B, so a
50% ratio is arithmetically impossible without removing compact content. The assertion now requires
at least 25% smaller (still a real bound) and says so. This is a test-policy consequence of the
projection, not a loosened capability check.

## Residual risk

- The advertised schema now under-documents the nested fields of `sources`/`fetches`. A client that
  generated a form from the old schema must paste prior output instead, which is the documented
  usage; the fields remain accepted.
- `$defs` shrinking is not a runtime change: the types, validation, dedup, linking, caps, and ID
  computation are untouched.

## Verification commands

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming   # 26 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test evidence_contract      # 13 passed
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test tool_surface_live      # 1 passed, 1 ignored
RUSTUP_TOOLCHAIN=1.89.0 make check                                                       # exit 0
RUSTUP_TOOLCHAIN=1.89.0 make docs-check                                                  # exit 0
```
