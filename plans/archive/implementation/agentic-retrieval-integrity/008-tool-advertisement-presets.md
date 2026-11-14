# M008 — Tool Advertisement Presets

Status: ready
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M008 (decision 4: tools may be hidden, never removed)
Primary class: capability
Baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a` (the worktree also carries uncommitted retrieval-defaults, scraper, and doc changes from the prior turn; re-read `git status` before starting and preserve them)
References: ADR-0001; `plans/subsystems/standalone-mcp-local-inference-roadmap.md`; `plans/archive/implementation/standalone-mcp-local-inference/001-opt-in-compact-tool-advertisement.md` and `plans/closure/standalone-mcp-local-inference/001-status.md`; `architecture/mcp.md`; `architecture/config.md`; `docs/tool-matrix.md`; `src/mcp/server.rs`; `src/mcp/tool_contract.rs`; `src/mcp/projection.rs`; `src/mcp/tools/common.rs`; `src/core/config.rs`; `src/core/workflow.rs`.
Dependencies: interface ADR-0001 (ten-tool contract). Soft: M001's opt-in compact profile, which this milestone composes with rather than replaces.

## Objective

Add a process-configured advertisement preset that selects **which subset of the existing ten tools appears in `tools/list`**, without changing the ten-tool contract, the dispatch router, argument validation, or any tool's computed payload. Presets compose with the existing `[mcp].schema_profile` byte-reduction knob instead of competing with it, and no preset may advertise a pointer to a tool the caller cannot see.

## Non-goals

- No tool added, removed, renamed, aliased on the wire, or split into a mega-tool. Dispatch keeps all ten names under every preset.
- No schema-breaking change: no required-argument change, no output-shape change, no new tool argument. Preset selection is a `tools/list` membership filter, not a schema edit.
- No change to what a tool computes or returns under the `full` preset, including `response_detail` defaults, brief budgets, and `output_truncated`/`omitted_count` semantics.
- No inference, model detection, per-client or per-request advertisement: the preset is resolved once from the process config, never from a request header, `User-Agent`, or MCP `_meta`.
- No new permission gate. A hidden tool stays callable; hiding is presentation, never authorization.
- No slideshow of presets from other products: exactly the four named below, defined as data.
- No inference of a preset from `schema_profile`, and no implicit preset selection. Absent configuration means `full`.

## Invariants that cannot regress

- Exactly ten canonical tool names exist and all ten are registered in the `ToolRouter` (`src/mcp/server.rs:401-537`, pinned by `tests/static_guards.rs:1182-1206`).
- `tools/call` never rejects a valid hidden tool. The router is built unfiltered at `src/mcp/server.rs:40`; only `advertised_tools` (`:41-45`) is filtered, and `#[tool_handler]` dispatches through `tool_router`, so a preset cannot disable execution by construction.
- `full` is the default and is byte-identical to the pre-milestone `tools/list`: the filter short-circuits for `full`, and the full-profile `tool_fingerprint()` is unchanged (pinned literal captured before editing).
- `schema_profile` keeps its existing semantics: `compact` still keeps all ten names when `tool_preset = "full"` and still omits `outputSchema`; `response_detail` still defaults to `diagnostic` under `full` and `brief` under `compact` (`src/mcp/projection.rs:31-39`). A `tool_preset` never changes the `response_detail` default.
- Every `next_actions` entry that survives filtering names a tool inside the active preset; every entry naming a hidden tool is dropped, never rewritten to another tool, never executed.
- Preset definitions are a static subset of the canonical names; a preset naming anything outside `tool_contract::ALL_CONTRACTS` fails a unit test, not a runtime path.
- Determinism: `tools/list` stays name-sorted (`src/mcp/server.rs:589`), and the content fingerprint stays FNV-1a over advertised bytes (no new ID semantics, no preset string hashed).
- Sanitization, trust framing, deadlines, provider ledger, and Mode-Off denial are untouched; advertisement is not a policy layer (`src/mcp/policy.rs` is not modified).

## Expected production changes

1. **Configuration.** Add `McpToolPreset { Full, Research, Code, Minimal }` next to `McpSchemaProfile` in `src/core/config.rs:101-105`, with `#[serde(rename_all = "lowercase")]` and `#[default] Full`. Add `pub tool_preset: McpToolPreset` to `McpSection` (`src/core/config.rs:107-114`) with `#[serde(default)]`, and set `Full` in `McpSection::default()` (`:120-127`). The three default sites that must agree are the enum's `#[default]` variant, `McpSection::default()`, and the `#[serde(default)]` field attribute; `architecture/config.md` § `[mcp]` and `docs/config.md` record the same value. No extra `validate()` rule is needed: an unknown string fails serde at load, which is the existing behaviour for `schema_profile`.
2. **Preset table and filters.** New ordinary module `src/mcp/advertisement.rs` (no ratchet needed; well under the 1,600-line / 80 KB ceiling). It owns the preset→tools table as data, the `next_tools` closure check, the `tools/list` membership filter, the payload pointer filter, and the preset-aware instruction renderer. The table must not live in `src/mcp/server.rs`: `tests/docs_tool_names.rs:14-28` and `tests/static_guards.rs:1203` count `name = "` occurrences in that file, and tool-name strings there would trip both guards.
3. **Server wiring.** `src/mcp/server.rs:37-53` resolves the preset once from `state.config.mcp.tool_preset`, applies it after `apply_compact_profile` (`:42-44`) so both axes compose, and freezes the result in `advertised_tools`. `get_info()` (`:541-547`) returns preset-rendered instructions instead of the single `EGGSEARCH_INSTRUCTIONS` constant (`:679-693`), which becomes a tagged line table rendered from the same module; the `full` rendering is byte-identical. A `tracing::info!` at construction records `preset`, `schema_profile`, advertised count, and hidden names for the operator.
4. **Response boundary.** Add one private `EggsearchServer::respond` seam that applies `advertisement::filter_payload` to the success value before `map_tool_result` (`src/mcp/tools/common.rs:328-343`), and route the ten `map_tool_result(...)` call sites in `src/mcp/server.rs:413-535` through it. This is the single choke point: `next_actions` are produced upstream by `src/meta/recipe_catalog.rs:126-319`, `src/core/workflow_coverage.rs:479-722` (including the nested `workflow_coverage.next_actions`), and adapter modules, so a per-producer change would be both wider and incomplete.
5. **Pointer filter rules.** `filter_payload` walks the JSON value and applies exactly four rules, counting drops: (a) array entries under any `next_actions` key keep only entries whose `tool` string is in the preset; (b) string arrays under `step_tools` keep only preset names; (c) object arrays under `steps` and `fallbacks` keep only entries whose `tool` string is in the preset, leaving `order` values unrenumbered so gaps stay visible; (d) recurse everywhere else. When at least one entry is dropped, the top-level object gains `next_actions_omitted: <u32>`, matching the existing `omitted_count`/`output_truncated` convention. Every advertised output schema is permissive (`additionalProperties: true`, `src/mcp/output_schema.rs:23`), so this is additive.
6. **Diagnostics.** `src/mcp/tools/provider_status.rs` gains an additive `mcp_advertisement` object in `build_payload_without_probe` (`:129-253`): `tool_preset`, `schema_profile`, `advertised_tools`, `hidden_tools`. No `_meta` key is added to `tools/list`: `rmcp 3.9` routes `_meta` through the SEP-managed `Extensions`/`MetaObject` machinery, strict clients may reject unknown keys, and `provider_status` is reachable under every preset because hidden tools still execute. Under `minimal`, which does not advertise `provider_status`, there is deliberately no in-band signal: the caller cannot discover the diagnostic tool from `tools/list`, and the contract is documented instead of echoed.
7. **Documentation.** `docs/config.md` § MCP Tool Schema Profile (preset key, values, default, precedence), `docs/tool-matrix.md` § Tool Disclosure Hints (preset table and the hidden-still-callable rule), `architecture/config.md` § `[mcp]` row, `architecture/mcp.md` § construction/list_tools/instructions, `docs/test-inventory.md` for the extended suites, and `CHANGELOG.md`.

The four presets, defined as data over the real canonical names:

| Preset | Tools (exact `tools/list` names) | Count |
|---|---|---|
| `full` (default) | `batch_fetch`, `build_evidence_bundle`, `provider_status`, `repo_fetch`, `repo_map`, `repo_search`, `research_search`, `security_search`, `web_fetch`, `web_search` | 10 |
| `research` | `provider_status`, `web_search`, `web_fetch`, `research_search`, `repo_fetch`, `batch_fetch`, `build_evidence_bundle` | 7 |
| `code` | `provider_status`, `web_search`, `web_fetch`, `repo_search`, `repo_fetch`, `repo_map`, `batch_fetch`, `build_evidence_bundle` | 8 |
| `minimal` | `web_search`, `web_fetch`, `batch_fetch`, `build_evidence_bundle` | 4 |

Each non-`full` preset is closed under `ToolContract.next_tools` for every tool it advertises, verified statically in work package 2: `web_search`→{`batch_fetch`,`web_fetch`}, `web_fetch`→{`build_evidence_bundle`}, `batch_fetch`→{`build_evidence_bundle`}, `repo_search`→{`batch_fetch`,`repo_fetch`,`repo_map`}, `repo_map`→{`repo_search`}, `repo_fetch`→{`build_evidence_bundle`}, `research_search`→{`batch_fetch`,`build_evidence_bundle`,`repo_fetch`,`web_fetch`}, `build_evidence_bundle`→{}, `provider_status`→{}. `security_search` is advertised only by `full`; a security caller uses `full`, optionally with `schema_profile = "compact"`, which is the existing byte knob. This closure is necessary but not sufficient: dynamic producers (`role_to_next_tool`, gap-driven coverage actions) can name any of the ten, which is why the runtime filter exists.

**Precedence, stated once and unambiguously:** `schema_profile` and `tool_preset` control different axes and never override each other. Projection order is fixed: contract metadata → `schema_profile` projection → `tool_preset` membership filter. `full`+`full` is the shipped default and byte-identical. `tool_preset = "minimal"` with `schema_profile = "full"` advertises four full-size schemas; `tool_preset = "full"` with `schema_profile = "compact"` reproduces M001 exactly.

## Storage, protocol, and compatibility effects

No persistence, no new cache dimension, no new file. `[mcp].tool_preset` is additive configuration: a config file without it keeps today's behaviour, and `McpSection` field additions are serde-defaulted so existing TOML still parses and `McpSection { ..Default::default() }` literals still compile (`tests/mcp_tool_contract.rs:486-496`).

Protocol effects are limited to `tools/list` membership and the `initialize` instruction text. `tools/call` wire behaviour, error codes, `repair` payloads, `outputSchema` presence, and all ten dispatch paths are unchanged. The fingerprint is content-derived over advertised definitions, so each preset yields a distinct stable fingerprint with no preset string hashed; clients that cache by fingerprint plus server version keep working and must not cache by tool count.

Two compatibility consequences are accepted explicitly. First, a caller that hardcodes the ten names and calls one hidden by the operator still succeeds. Second, a caller that enumerates only `tools/list` will never learn about a hidden tool; `provider_status.mcp_advertisement.hidden_tools` is the retrieval path when that tool is advertised, `tools/call` is the execution path when it is not.

## Ordered work packages

1. **Config surface.** Enum, `McpSection` field, all three defaults, `architecture/config.md` and `docs/config.md` rows. Extend `tests/config_validation.rs::default_config_has_expected_values` with `assert_eq!(cfg.mcp.tool_preset, McpToolPreset::Full)` and add a parse test for each accepted value plus a rejected-value test.
2. **Preset table and closure.** `src/mcp/advertisement.rs` with the four subsets, `tools_for(preset)`, `is_allowed(preset, name)`, and a closure assertion that every advertised tool's `ToolContract.next_tools` is a subset of the preset. Unit tests: every preset is non-empty, contains only canonical names, contains `web_search`, excludes nothing required, and is `next_tools`-closed. A new preset that is not closed fails here rather than shipping a dropped pointer.
3. **Advertised-list wiring.** Resolve the preset in `EggsearchServer::new`, filter after the compact projection, store `preset` and the allowed-name set, and add `pub fn tool_preset(&self)`. `tests/mcp_tool_contract.rs`: per-preset advertised name sets, name-sorted order, `tool_contract::tool_names()` still returning ten, distinct fingerprints per preset, identical fingerprint for two servers with the same preset, and the pinned full-profile fingerprint equal to the pre-change literal.
4. **Hidden-tool execution.** Extend `tests/mcp_http.rs` with a `minimal`-config server: assert the initialized `tools/list` payload has four names and omits `provider_status`, then issue a real `tools/call` for `provider_status` (`probe = false`, purely process-local) through the same helper pattern as `:202`/`:369-392` and assert a structured success, not an error. No handler changes. `tests/mcp_tools.rs` keeps the payload-level assertions. Do not use a network-touching hidden tool such as `repo_map` for this proof; tests stay offline.
5. **Payload pointer filter.** `filter_payload` with the four rules plus `next_actions_omitted`, and the `respond` seam at the ten call sites. Tests: a synthetic payload with hidden pointers in `next_actions`, nested `workflow_coverage.next_actions`, `step_tools`, and `steps`/`fallbacks` loses exactly those entries and reports the count; a payload whose pointers are all visible is returned unchanged and gains no new key; `full` is a strict no-op.
6. **Byte and shrinking proof.** Extend `tests/tool_surface_evaluation.rs` with `preset_byte_budgets`: serialize `tool_definitions()` for each preset at each schema profile through the same `serde_json::to_string` measurement used at `:477-495`, print per-preset totals, assert each non-`full` preset is strictly smaller than `full` at the same profile, and assert `minimal` at `compact` is under half of the ten-tool compact total measured in the same run (the recorded baseline is 18,545 bytes in `docs/retrieval-quality-context-efficiency.md` and 18,481 in the M001 plan, within the roadmap's 18.5–19.4 KB range; remeasure rather than trusting either figure, and put the exact per-preset figures in the closure record). The existing ten-tool assertion at `:455-459` keeps running on the default config.
7. **Diagnostics and docs.** `provider_status.mcp_advertisement`, asserted in `tests/mcp_tools.rs` under a non-`full` preset; assert preset-rendered instructions name no hidden tool and that the `full` rendering still fits the 2,000-byte instruction budget asserted at `tests/tool_surface_evaluation.rs:500-505`. Update the five documents and `CHANGELOG.md`, then refresh `docs/test-inventory.md` counts.

## Verification

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test mcp_http
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test tool_surface_evaluation
cargo test --locked --all-features --test docs_tool_names
cargo test --locked --all-features --test docs_config_snippets
cargo test --locked --all-features --test config_validation
cargo test --locked --all-features --test static_guards
python3 packaging/check-planning-consistency.py
make check
cargo test --locked --all-features
```

All suite names above exist under `tests/` at this baseline. Placement follows `architecture/testing.md`: the preset contract and size proof extend the existing `mcp_tool_contract` and `tool_surface_evaluation` suites; hidden-call behaviour extends `mcp_tools`; transport-level advertisement extends `mcp_http`; no new suite name is introduced, so `docs/test-inventory.md` needs counts, not new rows. `--all-features` enables `mock`, which the behavioral suites require.

Operator check, recorded rather than asserted, with `[mcp] tool_preset = "research"`:

```bash
eggsearch mcp serve --bind 127.0.0.1:11320
# log line: mcp advertisement preset=research schema_profile=full advertised=7 hidden=repo_map,repo_search,security_search
```

## Acceptance and stop conditions

- Default (`full`/`full`) `tools/list` and instructions are byte-identical to the pre-change output; the pinned fingerprint matches and every existing `tools/list` test passes unmodified.
- `research` advertises exactly 7 names, `code` 8, `minimal` 4, each in name-sorted order and each a subset of the canonical ten. A `tools/call` for a hidden name succeeds, proved end-to-end for `provider_status` under `minimal` over loopback HTTP, while `static_guards` continues to pin all ten registrations in `server.rs` so hiding cannot become deregistration.
- Every non-`full` preset is `next_tools`-closed in the static test, and the runtime filter drops and counts any pointer that escapes dynamic producers. Proven by unit tests in `src/mcp/advertisement.rs` over representative payload shapes (top-level `next_actions`, nested `workflow_coverage.next_actions`, `step_tools`, `steps`/`fallbacks`) plus a contract-driven test that applies the filter to each of the ten tools' `next_tools` sets under each preset and finds no surviving hidden name.
- Each non-`full` preset is strictly smaller than `full` at the same profile; `minimal`+`compact` is under half of the ten-tool compact total. Absolute bytes are recorded in the closure record, not asserted from memory.
- `provider_status` reports the active preset, schema profile, advertised names, and hidden names, and its recipe output never names a hidden tool as an executable step.
- `docs_tool_names`, `static_guards` (`stable_tool_registration_count_and_names` must still find exactly ten `name = "` in `server.rs`), `docs_config_snippets`, and `config_validation` stay green without weakening an assertion.

Stop and return for a decision rather than shipping a dead pointer if:

- a preset cannot be made `next_tools`-closed without adding a tool the operator did not select; or
- a proposed instruction line cannot be rendered without changing the `full` bytes; or
- a hidden tool's `tools/call` is ever rejected by the transport layer, which would falsify the "hidden still executes" invariant; or
- the `minimal` byte target requires changing a tool's schema, which is out of scope.

In every runtime case the filter fails closed: a pointer that names a hidden tool is removed and counted, never rewritten to a visible tool and never executed on the caller's behalf.

## Closure evidence

`plans/closure/agentic-retrieval-integrity/008-status.md` must record: the implementation candidate SHA; the pre-change `tool_fingerprint()` literal and its equality after the change; the per-preset × per-profile byte table with the measured ten-tool compact baseline; the exact commands and results above, including `make check`; the mock evidence that hidden tools execute under `minimal`; the `provider_status.mcp_advertisement` sample; the four-preset closure table; and residual risk — model confusion when a hidden tool is callable but invisible, preset drift when a future milestone adds or removes a tool (the closure test is the guard), the reduced diagnostic surface under `minimal`, and the fact that presets reduce advertisement bytes only, not response bytes.
