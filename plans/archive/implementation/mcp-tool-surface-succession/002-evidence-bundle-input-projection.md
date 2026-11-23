# M002 — Evidence-Bundle Input Projection

Status: closed
Date: 2026-11-22
Subsystem: MCP tool-surface succession
Milestone class: infrastructure + polish
Baseline: `b2abc59687c271a6e0723bdd77e4eeeef54bdb06` on `feat/auto-20261117-044c4885`
Decision authority: ADR-0001 (adapter ownership). No new ADR.

References: `plans/subsystems/mcp-tool-surface-succession-roadmap.md` §4, §7;
`plans/000-long-term-specification.md` §2; `docs/tool-matrix.md` (`build_evidence_bundle`);
`tests/mcp_schema_slimming.rs`.

Dependencies: none.

## Objective

Cut the advertised `build_evidence_bundle` input schema to a bounded passthrough projection
without narrowing the runtime types, changing accepted request JSON, or disturbing bundle
identity.

## Current evidence

- The tool's input schema is 39,543 B: 61.0% of the 64,850 B advertised input-schema total and
  roughly half of the serialized `tools/list`. Its top-level `properties` account for only
  1,214 B.
- The cost is 37,516 B of `$defs` across 39 definitions pulled in transitively by
  `EvidenceSourceInput` and `EvidenceFetchInput`: `RankReason` 3,572, `CodeEvidence` 2,814,
  `SourceMetadata` 2,666, `SourceKind` 1,887, `LocalRepoMatch` 1,555, `EvidenceFetchInput`
  1,526, `SourceRole` 1,428, `UncertaintyReason` 1,417, `QualityReason` 1,267,
  `RepoLocator` 1,259, `VulnerabilityMetadata` 1,254, and the rest.
- The nested types are load-bearing at runtime, not decorative.
  `src/meta/evidence_bundle.rs:116` reads `metadata.source_kind` (it feeds
  `compute_source_id`), `metadata.code_evidence.source_role`, `metadata.rank_reasons`, and
  `metadata.code.{host,owner,repo,path}` (structured repo follow-up). `fetch_input_to_bundle`
  reads `locator`, `selected_span`, `line_start`/`line_end`, the first 64 characters of
  `text` (for `compute_fetch_id`), `trust_markers`, and `warnings`.
- Stable IDs are content-derived and must not change. Narrowing the Rust types would change
  `source_id` inputs, so this milestone must not do that.

## Invariants that cannot regress

- `bundle_id`, `source_id`, and `fetch_id` are unchanged for every existing fixture and
  captured response.
- Accepted request JSON is unchanged: a caller may still paste a full source card or fetch
  result, and every field the builder reads today still arrives.
- `trust_summary`, `provider_summary`, deduplication, linking, caps, gaps, and warnings are
  unchanged.
- No output field is removed or renamed.
- No new dependency, provider, or tool.

## Work packages

1. **Project, do not narrow.** Keep `EvidenceSourceInput` and `EvidenceFetchInput` as
   deserialization targets. Replace the generated schema for the heavyweight optional fields
   with a bounded opaque object — `{"type":"object","additionalProperties":true,"description":
   "Pass this from a prior tool response verbatim."}` — using a schemars
   `schema_with` helper. Start with `EvidenceSourceInput.metadata`, `.quality`,
   `.trust_markers` and `EvidenceFetchInput.locator`, `.selected_span`, `.trust_markers`;
   verify which subset reaches the target budget and record the final set.
2. **State the contract.** Update the tool description and the `docs/tool-matrix.md` row so a
   caller knows to pass prior output verbatim and that nested metadata is accepted but not
   re-advertised. The parameter documentation for the advertised scalar fields stays.
3. **Budget.** Add a per-tool ceiling in `tests/mcp_schema_slimming.rs` for
   `build_evidence_bundle` and lower the overall `tools/list` budget by the measured
   reduction. The ceiling is set from the measured result, not guessed, and the closure record
   states both numbers.
4. **Identity regression.** Add a fixture that builds a bundle from a captured
   `web_search` card and a captured `fetch` result (including `metadata` and `locator`),
   asserts the resulting IDs and trust/provider summaries byte-for-byte against a
   pre-change snapshot, and runs with the projected schema.

## Expected production changes

| File | Change |
|---|---|
| `src/core/evidence_bundle.rs` | `schema_with` annotations and the opaque-schema helper |
| `src/mcp/tools/evidence_bundle.rs` | Description text only |
| `tests/mcp_schema_slimming.rs` | Per-tool ceiling and lowered total budget |
| `tests/` (evidence-bundle fixture) | Identity regression over a full card and fetch result |
| `docs/tool-matrix.md`, `architecture/mcp.md` | Verbatim-passthrough statement |

## Non-goals

- No change to the bundle builder, ID computation, dedup, linking, or caps.
- No removal of input fields and no change to their runtime parsing.
- No change to any other tool's schema.
- No claim that the removed `$defs` were unused; they stay used, only unadvertised.

## Compatibility and migration

Wire-compatible in both directions. Requests that validate today still validate; the schema
becomes more permissive for the projected fields. Clients that generate forms from the schema
lose nested field names for those six fields and must paste prior output instead, which is the
documented usage.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_schema_slimming
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tools evidence_bundle
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib evidence_bundle
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test evidence_contract
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Record `tools/list` bytes for this tool and for the whole surface before and after.

## Acceptance and stop conditions

- The tool's input schema is materially smaller and its new size is pinned by a ceiling.
- Every identity and summary fixture is unchanged.
- **Stop:** if a projected field turns out to need schema-level validation to keep a stable
  ID or to reject a malformed locator, leave that field advertised and record the exception
  rather than moving the validation into the handler.

## Required closure evidence

The closure record carries the before/after schema breakdown (`$defs` count and bytes,
top-level property bytes), the final projected-field set and why each remaining advertised
field stayed, the identity snapshot, the lowered budgets, the gates, and residual risk (schema
generation now under-documents six nested fields).
