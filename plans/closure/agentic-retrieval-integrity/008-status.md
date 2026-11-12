# M008 Tool Advertisement Presets — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/008-tool-advertisement-presets.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` §10 decision 4

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Result

`tools/list` membership is preset-filtered. Dispatch, argument schemas, and the ten-tool contract are
unchanged.

| Preset | Tools advertised |
|---|---|
| `full` (default) | `provider_status`, `web_search`, `web_fetch`, `repo_search`, `repo_fetch`, `repo_map`, `research_search`, `security_search`, `batch_fetch`, `build_evidence_bundle` |
| `research` (7) | `provider_status`, `web_search`, `web_fetch`, `research_search`, `repo_fetch`, `batch_fetch`, `build_evidence_bundle` |
| `code` (8) | `provider_status`, `web_search`, `web_fetch`, `repo_search`, `repo_fetch`, `repo_map`, `batch_fetch`, `build_evidence_bundle` |
| `minimal` (4) | `web_search`, `web_fetch`, `batch_fetch`, `build_evidence_bundle` |

Presets live as data in `src/mcp/advertisement.rs`. Every non-`full` preset is closed under
`ToolContract.next_tools` — enforced by `non_full_presets_are_next_tools_closed` plus a
contract-driven filter test across all ten tools × four presets — so a preset can never advertise a
tool whose follow-up suggestions name a hidden one.

## Configuration

`[mcp].tool_preset` in `src/core/config/mcp.rs`, default `full`, accepting `full`, `research`,
`code`, `minimal`. An unknown value is a serde parse error naming all four:

```
unknown variant `moderate`, expected one of `full`, `research`, `code`, `minimal`
```

Resolution order is fixed in `EggsearchServer::new`: contract metadata, then compact schema
projection, then the preset filter. `tool_preset` never changes the `response_detail` default —
`schema_profile` owns that — and `full` plus `full` is byte-identical to the pre-capability
advertisement (fingerprint pinned).

## Proofs

**Hidden tools still execute.** `src/mcp/server.rs` builds the `ToolRouter` from the unfiltered set,
so dispatch never consults the advertised copy.
`tests/mcp_http.rs::minimal_preset_advertises_four_tools_and_still_executes_hidden_tool` performs a
real loopback initialize, asserts `tools/list` returns four names and no `result._meta`, then calls
the hidden `provider_status` and gets `isError: false` with a populated providers array and the
correct `mcp_advertisement` block. At the router level, `EggsearchServer::routed_tool_names()`
returns all ten under `minimal`.

**`next_actions` stay callable.** Filtering happens at the single `EggsearchServer::respond` →
`advertisement::filter_payload` seam, wired at all ten call sites, covering top-level
`next_actions`, `workflow_coverage.next_actions`, `step_tools`, and `steps`/`fallbacks`. Unit tests
assert drop counts of 7 / 6 / 2 / 0 for `minimal` / `research` / `code` / `full`, that
`steps[].order` gaps are preserved, and that `next_actions_omitted` is absent when nothing drops.
The HTTP test exercises the seam over real `provider_status` recipes (summary `step_tools`, full
`steps` plus `fallbacks`). Entries are dropped and counted, never rewritten into another tool.

## Measured advertisement size

`serde_json::to_string(tool_definitions())`, full / compact serialized bytes:

| Preset | full | compact |
|---|---:|---:|
| `full` | 73,161 | 19,611 |
| `research` | 65,560 | 16,498 |
| `code` | 68,429 | 16,820 |
| `minimal` | 57,985 | 13,116 |

Fingerprints pinned: full `901d8bb6bfaab23e`, compact `cf538dbace2e20fb`.

## Unmet plan target, recorded rather than fudged

The plan asked for the compact `minimal` preset to reach under half of compact `full`. It lands at
66%. The remaining bytes are mandatory schema envelope, so no preset can reach half without changing
schemas — which the plan itself names as a stop condition. The measured numbers are documented in
`docs/config.md`, `docs/tool-matrix.md`, `architecture/config.md`, and `architecture/mcp.md` instead
of the original target being restated as achieved.

## Diagnostics

`tracing::info!` once at construction:
`mcp advertisement preset=… schema_profile=… advertised=… hidden=…`.
Additive `provider_status.mcp_advertisement` reports `tool_preset`, `schema_profile`,
`advertised_count`, `advertised_tools`, `hidden_tools`. There is deliberately no `tools/list._meta`
signal: `MetaObject` is SEP-owned and a strict client may reject unknown keys.

## Verification

- `cargo fmt --all`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `config_validation` 88, `mcp_tool_contract` 19, `mcp_tools` 7, `mcp_http` 13,
  `mcp_schema_slimming` 20, `docs_tool_names` 2, `static_guards` 61, lib 3,717, doc-tests 4 — all
  passed.
- Full sweep across 85 targets: 6,068 passed, 8 failed at the time of the run. All eight were
  attributed and individually re-run: one was this milestone's drop-count constant (fixed and
  re-verified), three were `engine_parse_conformance` in concurrent Startpage work (now 6/6), and
  four were doc-test artifact races (now 4/4).

## Deviations

- **`src/core/config/mcp.rs` is a new file.** The MCP config types were extracted from
  `src/core/config.rs` to respect its 1,900-line ratchet; the split is the ratchet's intended remedy.
- **One existing assertion was adapted.** The compact-profile HTTP test now compares
  `provider_status` payloads with the additive `mcp_advertisement` block removed, because the schema
  profile legitimately differs there now. The comparison's intent is preserved.

## Residual risk

- A host that caches `tools/list` across config changes will keep stale membership until it
  re-lists. The preset is server configuration, not a protocol negotiation.
- A model behind `minimal` cannot discover specialists at all. `provider_status` is advertised in
  every preset except `minimal`, and `minimal` deliberately keeps only the core primitives plus the
  bundle tool.
- Preset size reductions are modest in compact mode because schema envelope dominates; the larger
  win is in `full` mode (73,161 → 57,985).
