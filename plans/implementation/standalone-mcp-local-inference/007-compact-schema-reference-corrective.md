# M007 — Compact Schema Reference Corrective

Status: closed (candidate `772a073fd40e9f044017b26bae18a6efd0724b1d`; hosted CI `41220450246`)
Primary class: corrective
Source roadmap: `plans/subsystems/standalone-mcp-local-inference-roadmap.md`
Corrects: M001 compact tool advertisement, identified during M003 small-model API qualification
Related plans: `plans/implementation/standalone-mcp-local-inference/001-opt-in-compact-tool-advertisement.md`; `plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`
Related closures: `plans/closure/standalone-mcp-local-inference/001-status.md`; current M003 evidence in `plans/closure/standalone-mcp-local-inference/003-status.md`

## Objective

Make every compact `tools/list` input schema self-contained with all local `$ref` references resolvable, preserving compact tool behavior, all ten tool names, explicit `response_detail` support, and the serialized-size budget.

## Evidence and why earlier qualification missed it

An all-tools compact LM Studio native API turn failed constrained decoding with `cannot resolve $ref #/$defs/ResponseDetail, $defs not found`. Inspection of the exact compact MCP schemas found that `build_evidence_bundle` manually replaces nested source/fetch schemas and then removes the entire root `$defs` object, while retaining `response_detail` as a property referencing `#/$defs/ResponseDetail`.

Earlier compact schema tests verified allowed properties, enum values, names, and byte budgets. They did not resolve every local reference after per-tool schema rewriting; the M003 API tests had also restricted `allowed_tools` to one specialist tool and therefore did not exercise the failing ten-tool palette.

## Scope and invariants

- Change only compact input-schema serialization and its regression coverage.
- Preserve all ten tool names, accepted runtime arguments, default behavior, validation, and MCP result contracts.
- Keep explicit `response_detail` advertised and preserve the compact byte budget.
- Do not remove a parameter or weaken runtime validation to hide schema incompatibility.

## Work packages

1. Preserve the `ResponseDetail` definition or inline its supported values when compacting `build_evidence_bundle`; remove only definitions that have no remaining references.
2. Add a test that walks all ten compact schemas and asserts every local JSON Schema reference resolves against that schema root.
3. Run the focused schema suite and canonical local verification. Repeat the native small-model API task with all ten tools exposed and no `allowed_tools` filter; confirm the full failure is an input-context limit and compact reaches a valid tool call without a schema error.
4. Update M003 evidence, this closure record, the roadmap, and registry. Keep M003 open until the broader small-model task matrix is evaluated.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_schema_slimming
make docs-check
```

The native model API comparison is opt-in operational evidence and must not become a network-dependent CI test.

## Acceptance

- All local `$ref` values in every compact tool input schema resolve against the advertised schema root.
- All ten tools remain advertised and compact schema bytes remain within the existing limit.
- The all-tools compact API trial can initialize and invoke the task-appropriate tool without the unresolved-reference error.
- Exact candidate, API result, local tests, and restored `mcp.json` hash are recorded in `plans/closure/standalone-mcp-local-inference/007-status.md`.
