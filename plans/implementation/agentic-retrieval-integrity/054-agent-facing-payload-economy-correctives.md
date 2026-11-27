# M054 — Agent-Facing Payload Economy and Brief-Projection Correctives

Status: active
Date: 2026-11-26
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective program)
Baseline: commit `b04e996` (`feat/auto-20261126-82ca75e6`, release build 0.4.5, macOS arm64)
Corrects: M052 (`plans/implementation/agentic-retrieval-integrity/052-agent-facing-contract-ergonomics.md`), M039 (`plans/archive/implementation/agentic-retrieval-integrity/039-fetch-emitted-payload-budget.md`), M042 (`plans/archive/implementation/agentic-retrieval-integrity/042-fetch-item-shape-diagnostics.md`)
Closure of record for the corrected surface: `plans/closure/agentic-retrieval-integrity/052-status.md`
Trial evidence: live stdio MCP sessions against the release binary, nine canonical tools plus three aliases, more than 90 tool calls including repeats, error probes, local-only mode, PDF extraction, browser-rendered fetches with screenshots, and four non-default build variants (`--no-default-features`, `+documents`, `+egress`, `--all-features`); artifacts under `/tmp/egg-trial/` on the trial host (not committed).

Companion plan: M055 (`plans/implementation/agentic-retrieval-integrity/055-fetch-extraction-completeness-correctives.md`) owns extraction completeness and render-path status; this plan stays the payload, schema, error, and wire-hygiene programme.

Decision authority: ADR-0022 (agent-facing contract), AGENTS.md § Conventions (bounded I/O, content-derived IDs), `plans/003-planning-process.md` §7 (a corrective pass is a new plan).

## Implementation status (2026-11-26, partial)

Status is `active`: WP0 landed with tests; WP2-WP6 landed in part; WP7 is done; WP1 and WP8 are not started.

| WP | State | What landed | What remains |
|---|---|---|---|
| WP0 | done | `project_brief` clips text to a 512-character floor and keeps the record; a record is removed only after clipping is exhausted; a removal is counted and named in `warnings` and in a typed `fetch_content_truncated` entry inside a reserved notice budget; inherited `omitted_count`/`output_truncated` survive re-projection (that reset produced the observed `omitted_count: 0`); `chars_returned` and `total_chars_returned` are recomputed from the emitted text | the proptest for the `fetched > 0 && results.is_empty() && omitted == 0` invariant |
| WP1 | not started | - | canonical representation per item and the payload-ratio ceilings |
| WP2 | partial | `provider_status` gained `response_detail`; its `brief`/`compact` view is asserted under 6,000 characters; `recipe_detail` advertises the `summary` default | the default-detail move from `diagnostic` to `compact`, the `retrieval_summary.dimensions` collapse, the `suggested_fetches`/`next_actions` caps, and the `repo_map` default entries |
| WP3 | partial | `output_schema` rewritten against the keys the tools really return; `fetch.items` described; tool-contract text adjusted; schema tests updated | the 6,000-character `fetch` schema and the 26,000-character `tools/list`; the capability-aware `--no-default-features` schema and the single authorised fingerprint re-pin landed |
| WP4 | partial | cause-based classification through `fetch_tool_error` (capability, caller, policy, interaction, upstream), the duplicated `HTTP error: 404 HTTP 404` text collapsed, the `documents` rebuild hint, browser-profile hint keys corrected, an unknown profile names `eggsearch browser-login <origin> --profile <name>` | the uniform rmcp deserialization envelope in `server.rs`/`common.rs` and its nine-tool test |
| WP5 | partial | `recipe_detail` default aligned; `repo_search` names the accepted forms in its rejection and description; `brief` is documented in `docs/tool-matrix.md` | a `repo_map` local selector |
| WP6 | partial | sentinel-frame body trimmed, `score` published to four decimals, `completion_confidence` published as exact `f32`, timestamps keep source precision instead of gaining a nanosecond tail | one identifier per card, and warning deduplication, collapse, and severity |
| WP7 | done | default filter directive added in `src/main.rs`, targeting `chromiumoxide::handler` and composed with `RUST_LOG`; default-running subprocess regression renders the deterministic mock fixture with `RUST_LOG` unset | on macOS Chrome 157, one render measured 8 stderr lines / 1,415 bytes; no `WS Invalid message` warnings; regression skips only when Chrome/Chromium is unavailable |
| WP8 | not started | - | `max_chars_per_item` on the compact PDF path, `total_pages`/`pages_returned` |

Deviations forced by evidence:

- WP2's `provider_status` slimming is opt-in, not the default. Slimming the default broke 27 contract tests in `tests/provider_routing.rs` that encode the full diagnostic report (`enabled`, `configured`, `health`, `workflow_recipes`, parse shape). The default payload is unchanged and the 6,000-character ceiling is asserted on `response_detail: "brief"`.
- WP6 rounds `score` to four decimals as drafted but publishes `completion_confidence` as its exact `f32` instead of rounding to three decimals: rounding broke `core::workflow_coverage::tests::serialization_roundtrip`, which asserts exact round-trip equality. The exact `f32` is shorter than the widened `f64` and lossless.
- WP3 landed the fidelity half and not the size half. `fetch.items` carries a description, and the three selector-honest tool descriptions now state their accepted forms, so the advertised surface is re-pinned to `d7968115e378f708` / 34,226 bytes (from `678e2a042ab5b39b` / 33,983). That is a 243-byte increase: no reduction was achieved, and the 26,000-byte target is unmet. Dropping `outputSchema` entirely would have reached 30,229 bytes and is authorised by this plan, but it deletes a contract enforced by `tests/mcp_2026_protocol.rs` and `tests/mcp_tool_contract.rs` and documented in `docs/tool-matrix.md` to save 3,754 bytes, so it was reverted. The `fetch` input schema grew by 115 bytes to 10,456 for the new `items` description; the `advanced` option grouping that was meant to bring it to 6,000 was not done. Both `tests/mcp_schema_slimming.rs` guards were raised to the new measured sizes (definition 11,427 and input schema 10,456) rather than left failing, and that is a ratchet increase, not a reduction.
- `repo_map`'s default entry budget was lowered to 20 and then reverted: the reduction dropped the README suggestion that `corpus_repo_map_returns_structure` asserts, and a map that stops suggesting the README is worse than the payload it saves. The ceiling remains open.
- `repo_search`'s rejection for a `symbol`-only request now names the accepted forms (a non-empty `query`, or a locator with both `owner` and `repo`) while keeping the `invalid query:` prefix that the corpus contract asserts.
- Toolchain: the host default is Rust 1.100 while `rust-version` and CI pin 1.89. `make check` with the default toolchain fails clippy on untouched files (`manual_is_multiple_of`, `chunks_exact_to_as_chunks`, `needless_bool`), so the gate was run as `RUSTUP_TOOLCHAIN=1.89.0 make check`, which passes end to end: fmt, clippy with `-D warnings`, the no-default-features check, the all-features test suite, hygiene guards, dependency policy, and the packaging contract.
## Review corrections (2026-11-26)

An independent review of the landed changes found four defects and four documentation drifts. All are corrected in the same working tree, and the canonical gate was re-run afterwards.

- WP0: the omission notice also emits a typed `fetch_content_truncated` entry in `structured_warnings` when it fits, and 512 characters are reserved for the notice before the first record is removed, so the warning itself cannot be dropped to satisfy the budget (`src/mcp/projection.rs`; asserted in `tests/mcp_projection.rs`).
- WP3: the `outputSchema` contract loop deleted alongside the reverted schema removal is restored (`tests/mcp_tool_contract.rs`). Presence was still enforced by `tests/mcp_2026_protocol.rs`; the no-replay-cursor scan over output schemas was not.
- WP7: the directive targeted `chromiumoxide::handler::websocket`, which is not a module in chromiumoxide 0.9.1 — the warning is emitted from `chromiumoxide::handler` (`handler/mod.rs:629`) — and `EnvFilter::try_from_default_env` discarded the directive whenever `RUST_LOG` was set. The directive now targets `chromiumoxide::handler` and is appended to `RUST_LOG` (`src/main.rs`).
- WP4b: the `documents` rebuild hint no longer lists `text/csv`, because CSV extraction works without the feature.
- WP2: `response_detail: "compact"` is labelled `compact` rather than `brief`.
- WP5: the `symbol`-only rejection is decided from the request shape rather than by substring-matching the validator's prose.

## Objective

Restore the agent-facing promise that a tool call returns usable evidence inside a
bounded context budget. The trial found one release-blocking projection defect and a
cluster of payload/schema defects that make the recommended low-context configuration
return nothing at all, and make default-detail responses 40–65 KB for single calls.

This plan records the evidence and authorises no code change. It does not reopen
M039's emitted-payload invariant; it corrects the *default representation*, the
*brief projection*, and the *advertised schema* around it.

The trial's second pass added PDF and browser-rendered fetches and exercised every
non-default build option. Those results extend the evidence and add WP8 for caller
budgets that only some content paths honour; extraction completeness and browser status
handling are deliberately routed to M055.

## Evidence

All sizes are characters of the JSON text content of one `tools/call` response,
measured on the release binary with keyless default providers.

| Finding | Measured evidence | Source | Why prior verification missed it |
|---|---|---|---|
| F1 (P1) `fetch` under `brief` silently drops successful items. A single item larger than `brief_max_chars` (8,000) yields `results: []`, `returned_count: 0`, `omitted_count: 0`, `failed: 0`, no warning, while `fetched: 1` and `total_chars_returned: 9146`. With two items, one is dropped and still reported as `omitted_count: 0`. Because `[mcp].schema_profile = "compact"` makes `brief` the default, **the compact profile returns no fetched content for any default `fetch` whose single-item payload exceeds `brief_max_chars`** — which was every real page tried in the trial. The same seam drops a PDF: RFC 9110 (194 pages) under `brief` returned `results: []`, `omitted_count: 0`, `warnings: []`, with `total_chars_returned: 12,209`. | 353 chars returned for `blog.rust-lang.org/2025/08/07/Rust-1.89.0`; 1 of 2 items dropped; `max_chars_per_item: 2000` restores the item (2,178 payload), proving size-triggered removal | `src/mcp/projection.rs:194-330` (`project_brief` removes whole primary records), `src/mcp/projection.rs:35-38` (compact profile default = brief), `src/mcp/tools/batch_fetch_budget.rs:82` | M052 tested advertised metadata and explicit-detail responses; no test asserted `fetch` output under `brief`, and the compact profile was validated for advertisement size, not response content |
| F2 (P1) One `fetch` item is returned as several copies of the same content. | Web item: `text` 4,154 + `document` 13,115 = `payload_chars` 18,656 for `chars_returned` 4,538 (4.1×). Repo item: `text` 12,461 + `lines` 24,989 = `payload_chars` 38,208 for 12,280 chars. `max_chars_per_item` (12,000) bounds extraction, not the item payload. PDF under `diagnostic` is worse: `text` 3,092 plus `document.blocks`, `document.chunks`, and `document.outline` reach `payload_chars` 26,604 for 3,000 requested chars (8.6×). | `src/mcp/tools/batch_fetch.rs:73`, `src/core/fetch_policy.rs` focus path | M039 bounded the aggregate payload and required `payload_chars` to be honest — both hold. Nobody measured the default *shape* ratio (payload ÷ readable text) |
| F3 (P2) Specialist defaults are 40–65 KB per call. | `repo_search` 41,785 (`compact` 32,616); `research_search` 49,002 (`compact` 30,438); `security_search` 63,112 CVE / 57,608 package (`compact` 43,548); `provider_status` 58,275, probe 65,208 (`providers` block alone 47,291); `repo_map` 22,392 at 40 entries and 470,610 at the default cap; `knowledge_brief` 2,913 is the positive control. Telemetry dominates: `retrieval_summary.dimensions` 5,488, `workflow_coverage` 3,359–4,311, `suggested_fetches` 3,337–4,148, `next_actions` 2,186–2,441. | `src/mcp/projection.rs:28-38` (full profile default = diagnostic), `src/mcp/tools/*` | The trial measured default-detail responses for the first time; matrix/docs describe fields but not default cost |
| F4 (P2) Advertised schema is expensive and partly inaccurate. | `tools/list` 36,178 full / 23,227 compact (−35.8%) / 19,793 minimal preset, of which `fetch` is 12,034 (61% of minimal). `fetch` input schema 11,007 chars, `$defs` 9,637 across 15 definitions, and `items` carries no description. `outputSchema` is 284–595 chars but `web_search` advertises 7 of the 17 real top-level keys and includes `excerpt_economy`, absent from the response. | `src/mcp/tool_contract.rs`, `src/mcp/output_schema.rs:115-134`, `src/mcp/tools/fetch.rs:17-45` | M052 pinned the fingerprint but did not audit schema completeness against a real response, nor schema cost per tool |
| F5 (P2) Two incompatible error shapes. | Missing required argument → plain text `failed to deserialize parameters: missing field 'query'` (`isError: true`, no code). Semantic failure → `{"code":"invalid_semantic_value","message":"items must not be empty"}`. No error carried a repair hint. | rmcp argument deserialization seam vs `src/mcp/tools/common.rs` `ToolError`/`RepairHint` | Existing tests assert error *codes* for semantic failures only; deserialization failures short-circuit before the taxonomy |
| F6 (P3) Selector semantics contradict the schema. | `repo_search {"symbol":"finalize_scrape"}` is rejected: "requires a non-empty query or a repository locator". `repo_map` requires `owner`+`repo` and has no local selector, so a checkout without a forge remote cannot be mapped. `provider_status.recipe_detail` states the default is summary while the schema advertises `default: null`. | `src/mcp/tools/repo_search.rs:47-51`, `src/mcp/tools/repo_map.rs:16-19`, `src/meta/local_inventory.rs:173-186,768-776` | Selector tests cover the accepted shapes, not the rejected natural ones |
| F7 (P3) Wire hygiene noise. | Every card repeats `id` and `stable_id` (44 bytes/card); `score: 0.048915917503966164`, `completion_confidence: 0.5000000596046448`; timestamps carry nanoseconds; sentinel-wrapped strings keep the newlines inside the frame. | `src/core/source_card.rs`, `src/core/sanitize.rs` | Not previously measured; correctness is unaffected |
| F8 (P2) Server log spam can stall a stdio host. | A single browser render emits 1,330 `WS Invalid message: …` warnings (85 KiB stderr). A host that does not drain stderr blocks once the 64 KiB pipe buffer fills; this was reproduced as an indefinite tool-call stall and re-attributed to the undrained pipe only after the same call returned in 8.1 s with stderr drained. | `chromiumoxide-0.9.1/src/handler/mod.rs:629`, `src/main.rs:425` | Log volume was never part of the MCP trial matrix; a later render measured 3,570 bytes / 17 warnings, so the ceiling is a target, not a constant |
| F9 (P2) Caller character budgets are honoured by some content paths and not others. | PDF under `compact`: `max_chars_per_item: 3_000` returned a 12,092-char `text` (`chars_returned` 12,209, `payload_chars` 14,421) with `truncated: false`; with `pdf.pages: "1-3"` and `max_chars_per_item: 4_000` it returned 4,628. The same request under `standard` and `diagnostic` returned exactly 3,000 chars with `text_truncated: true`, so the defect is compact-only. No detail level reports `total_pages` or `pages_returned`; the 194-page count surfaced only in an out-of-range error. PDF caps come from `pdf_max_chars_per_page` 12,000 and `pdf_max_total_chars` 50,000. | `src/mcp/projection.rs`, `src/fetch/limits.rs:200-210`, `src/mcp/tools/batch_fetch.rs:405-420` | Budget tests assert the aggregate and HTML per-item caps; no test asserts `max_chars_per_item` on a PDF at each detail level |
| F10 (P2) Warnings are duplicated and mis-levelled. | One PDF fetch emitted 5 `structured_warnings` for 3 facts: `page selection applied: 1, 2, 3` twice (generic `fetch_warning` plus typed `pdf_page_selection_applied`), `PDF outline truncated at 200 entries` twice, plus trust. The generic twins carry the no-op action "Fetch-layer warning; review the original message for details.", and an explicitly requested, successful page selection is severity `warning`. `warnings` repeats the same messages as plain strings. A Wikipedia fetch emitted 8 warnings, 6 identical "table has irregular row lengths". | `src/core/warning.rs`, `src/fetch/pdf.rs`, `src/mcp/tools/batch_fetch.rs` | Warning tests assert that codes exist; none asserts uniqueness, severity, or the duplicated bytes |
| F11 (P2) The advertised schema ignores compiled capability. | With `--no-default-features`, `fetch`'s input schema is 10,341 chars and still advertises `pdf`, `render`, `include_screenshot`, and `browser_profile` — byte-identical to the default build — while `provider_status` honestly reports `compiled: false` for pdf, documents, and browser. An agent learns an option is unusable only by calling it. | `src/mcp/tools/fetch.rs:50-111` (no cfg gating), contrast `src/mcp/tools/web_fetch_options.rs:99,123` | Schema tests run on the default feature set only |
| F12 (P3) Capability errors are strong but mislabelled and asymmetric. | Without `pdf`: `upstream_failed: pdf_not_compiled_in: PDF support is not compiled in; build with cargo build --features pdf (see docs/features.md#pdf-extraction …)` — exact and actionable, but prefixed `upstream_failed` though nothing upstream failed. Without `documents`: `upstream_failed: unsupported_content_type: unsupported content type: application/vnd…wordprocessingml.document` with no rebuild hint. The `+documents` build extracts DOCX (heading, bullets, table cells) and XLSX (cells); CSV works even on the default build. | `src/fetch/client.rs:607,774,1039`, `src/mcp/tools/batch_fetch.rs` error mapping | Capability-off builds are compile-checked, never exercised for message quality |
| F13 (P3) Repair hints are not executable. | `browser_profile` without persistent profiles returns "set [fetch.browser].persistent_profiles_enabled = true"; the real key is `[fetch.browser.persistent_profiles].enabled`, the flat key is silently ignored, so following the hint changes nothing. With the key corrected, an unknown profile returns `manual_interaction_required` with message `browser_profile: trial-prof` and no pointer to `eggsearch browser-login <origin> --profile <name>`. `architecture/commands.md:275,328` repeat the wrong key while `docs/features.md:628` has the right one. | `src/mcp/tools/web_fetch.rs:437-438`, `src/mcp/tools/web_fetch_options.rs:169-170`, `src/fetch/browser/profiles.rs:106`, `src/core/config.rs:466-479,560` | Hints are prose; no test resolves a named config key against `AppConfig` or a named command against the CLI |
| F14 (verified) Browser rendering, screenshots, and lifecycle behave as documented. | `render: "browser"` reports `transport: "browser"`; `include_screenshot` returned an image block (75,924 base64 chars ≈ 57 KB PNG) in 5.7 s; an invalid `[fetch.browser].executable` yields `browser_unavailable: configured browser executable is invalid: …` with no fallback; loopback is blocked under browser even with `allow_localhost = true`; a clean shutdown created no new Chrome processes; each render logs "Browser was not closed manually, it will be killed automatically in the background". | `src/fetch/browser/navigate.rs`, `src/fetch/browser/lifecycle.rs`, `src/fetch/browser/discover.rs:74-87` | n/a — verification |

Default-feature verification: `Cargo.toml` sets `default = ["pdf", "browser"]`;
`release/eggpack/build-bindings.toml` states the published distribution uses the crate's
default feature set and requests no explicit flags; `README.md:139` and
`docs/installation.md:53` document both as enabled in source builds; and `provider_status`
on the release binary reports `pdf.compiled: true`, `browser.compiled: true`,
`documents.compiled: false`. No default change is proposed. `documents` stays opt-in
(both `pdf` and `browser` are already default); this plan only requires its capability
error and schema to be as honest as the PDF ones.

Confirmed positives to preserve: `next_actions[0].input_template` executed verbatim
without reconstruction (search → fetch → bundle in 3 calls); `knowledge_brief`
stayed at 2,913 chars; local-only mode (`[search].mode = "off"` + `[local]`) answered
in 642 ms at 14,946 chars; semantic error envelopes are clean; hidden tools stay
callable under a preset; compact schema reduces advertisement by 35.8%. PDF extraction
is otherwise strong (metadata title, page ranges, reversed ranges normalised, page 0
rejected as a semantic error naming the one-indexed rule, out-of-range rejected with the
real page count, and `%PDF-` magic detection when the server says octet-stream); the
`+egress` build routed provider upstreams through a local proxy (three CONNECTs) while
`fetch` produced none, and the default build rejected an enabled route with a rebuild
hint; the `+documents` build extracted DOCX and XLSX content exactly.

## Contract and invariants

- Keep nine advertised tools, three compatibility aliases, argument sets, provider
  ids, default provider set, trust framing, and content-derived IDs unchanged.
- Preserve M039: `max_total_chars` still bounds the emitted payload, and
  `document`/`lines` degrade before requested text is clipped. This plan changes which
  representations are emitted *by default*, not the budget invariant.
- A successful item must never disappear without a machine-readable record: either it
  is present with clipped content, or it is absent with `omitted_count` incremented,
  `failed` accounting, and a warning naming the budget and the item.
- `payload_chars` must remain the true emitted size for an item, including every
  representation this plan keeps.
- The advertised surface changes only in WP3; exactly one fingerprint re-pin is
  authorised, and its old/new values and byte delta must be recorded in the closure.
- No new provider, no default enablement, no network requirement in tests.

## Work packages

### WP0 — Brief projection must clip, not drop (P1, S)

Make `brief` a lossy *view* of a successful response, never a filter that removes the
only evidence. Where a record exceeds the remaining budget, clip its text fields to the
remaining allowance, mark `truncated`, and keep the record. Only when a record cannot
retain a usable minimum (path/URL + some text) may it be dropped, and then
`omitted_count` must count it and a warning must name it. The first test must also
confirm which seam performs the drop (brief projection or the fetch budget path);
the observed `omitted_count: 0` is consistent with either. `total_chars_returned` must
describe content actually returned.

Tests: extend `tests/mcp_tools.rs` with a fixture whose single item exceeds
`brief_max_chars` (assert 1 result, clipped text, `returned_count 1`,
`omitted_count 0`, warning present); a two-item fixture where one is dropped (assert
`omitted_count 1`); and a compact-profile end-to-end call (assert non-empty results).
Add a proptest that no input produces `fetched > 0 && results.is_empty() && omitted == 0`.

### WP1 — One canonical representation per fetch item (P1, M)

Default `standard` detail emits `text` plus locator/span metadata and omits
`document` and `lines` unless the request asks for structure (`focus`,
`offset`/`limit`, container documents) or `response_detail = "diagnostic"`. Repo
items return the selected window in `text` with `selected_span`; they do not also
return the full `lines` array. Every emitted representation counts toward
`payload_chars` and the per-item budget.

Acceptance: web item payload ≤ 1.5× `chars_returned`; repo item payload ≤ 1.6×;
no duplicate body under default detail; `compact` and `brief` stay loss-free apart
from explicit clipping.

Tests: extend `tests/web_fetch_integration.rs` and `tests/evidence_contract.rs` with
fixture assertions on emitted shapes and a ratio ceiling; keep the M039 budget tests
green to prove the invariant did not move.

### WP2 — Agent-safe default detail and telemetry collapse (P2, M)

Move the full-profile default from `diagnostic` to `compact` (the mode the docs already
recommend for ordinary turns), and make `diagnostic` the only place that carries
per-dimension telemetry. Collapse `retrieval_summary.dimensions` to counts plus at most
one line per dimension; cap `suggested_fetches` and `next_actions` at their highest-priority
entries with an explicit `omitted` count; give `provider_status` a `response_detail`
switch (or a routable-only default) so the 47 KB capability matrix is opt-in; lower
`repo_map`'s default entry budget.

Acceptance, measured on the trial fixtures: every search tool ≤ 12,000 chars at default
detail; `provider_status` ≤ 6,000 without `probe`; `repo_map` ≤ 12,000 at default
entries; `knowledge_brief` unchanged.

Tests: snapshot assertions of emitted size per tool against fixtures; a regression that
`diagnostic` still carries the full telemetry tables.

### WP3 — Advertised schema cost and fidelity (P2, M)

Describe `fetch.items` (one object or a non-empty array; `type` is `web`, `repo`, or
`site`). Reduce the `fetch` input schema by moving rarely used option groups into a
single referenced `advanced` object, targeting ≤ 6,000 chars (−45%). Make
`outputSchema` match the top-level keys the tool really returns, or drop `outputSchema`
entirely; do not advertise fields a response will not contain. State the
at-least-one-selector requirement for `build_evidence_bundle` and `security_search` in
prose.

The schema must also agree with the build: either omit uncompiled options from `fetch`
(or mark them in the tool description) or state availability only through
`provider_status`, but never advertise an option whose every call returns
`capability_unavailable`. Whichever is chosen, capture `tools/list` from
`--no-default-features` and from the default build and assert the intended difference —
today both are 10,341 chars and identical.

Acceptance: full `tools/list` ≤ 26,000 chars with all nine tools and required fields
intact; exactly one fingerprint re-pin recorded with old/new values; the build-variant
assertion above passes.

Tests: `tests/docs_tool_names.rs` stays green; add an advertised-vs-emitted key test
that a captured response's top-level keys are a subset of the advertised properties.

### WP4 — Uniform argument error envelope (P2, S)

Route rmcp deserialization failures through the same `ToolError` conversion seam so
every rejected call returns `{code, message, field, accepted, repair}` with
`isError: true`. Keep `-32602` for an unknown tool name.

Classify by cause rather than by layer: only genuine upstream failures may use the
`upstream_failed` prefix. Build-capability misses (`pdf_not_compiled_in`,
`browser_not_compiled_in`, and a new `documents_not_compiled_in`) and caller errors
(`pdf_page_out_of_range`) get their own codes; today both arrive as `upstream_failed`.
Give `documents` the same rebuild hint `pdf` already has. Also collapse the duplicated status text in HTTP errors, which today reads `upstream_failed: http_status: HTTP error: 404 HTTP 404`.

Make every hint executable. A hint that names a config key must resolve against
`AppConfig` (`[fetch.browser.persistent_profiles].enabled`, not
`[fetch.browser].persistent_profiles_enabled`), a hint that names a command must exist in
the CLI (`eggsearch browser-login <origin> --profile <name>`), and an unknown
`browser_profile` must not surface as `manual_interaction_required` with the profile name
as its only text. Correct `architecture/commands.md:275,328` to match
`docs/features.md:628`.

Tests: for each of the nine tools, a missing-required and a wrong-typed call assert the
envelope and a non-empty repair hint; assert no plain-text `failed to deserialize`
payload remains; assert `pdf_page_out_of_range` is not `upstream_failed`; add a test that
extracts bracketed TOML key paths from user-facing error strings and resolves each
against `AppConfig`, and one that parses every named subcommand.

### WP5 — Selector semantics and default alignment (P3, S)

Either accept `symbol` alone for a local workspace request, or state in the advertised
description that `symbol` requires `query` or a locator; make the rejection message name
the accepted forms. Add a local selector to `repo_map` (or state the forge-identity
requirement in the description and `docs/tool-matrix.md`). Align
`provider_status.recipe_detail`'s advertised default with its behaviour, and document
`brief` in `docs/tool-matrix.md`.

### WP6 — Wire hygiene (P3, S)

Trim whitespace inside sentinel frames, emit one identifier per card (`id` with
`stable_id` kept only where it is a distinct contract), round `score` to 4 decimals and
confidence to 3, and truncate timestamp fractions to milliseconds. Report the measured
byte delta in the closure.

Emit one warning per fact. A typed code replaces its generic `fetch_warning` twin instead
of accompanying it; routine acknowledgements of an explicit request ("page selection
applied: 1, 2, 3") are informational, not `severity: warning`; repeated identical
warnings collapse into one entry with a count; and `warnings` stays the short string form
of `structured_warnings` without boilerplate actions such as "Fetch-layer warning; review
the original message for details."

Acceptance: a PDF fetch with three facts emits at most three warnings with no duplicate
message text; a Wikipedia fetch with six identical table warnings emits one with a count;
routine acknowledgements no longer appear as warnings.

### WP7 — Server log hygiene for stdio hosts (P2, S)

Add a default filter directive that silences the chromiumoxide websocket warning class
(or its specific message) and rate-limit any remaining repeated warning per render.

Acceptance: one browser render emits fewer than 20 stderr lines and under 4 KiB (the
later pass measured 17 lines / 3,570 bytes for one render; the earlier pass measured
1,330 lines / 85 KiB, so both the page and the session matter).

Test: `tests/browser_transport.rs::stdio_browser_render_stderr_is_bounded_by_default`
launches the actual MCP stdio binary, renders the mock-intercepted fixture, clears
`RUST_LOG`, and asserts the line/byte limits plus absence of the websocket warning. It
runs without an environment opt-in when `mock` is enabled and skips only when Chrome or
Chromium is unavailable.

### WP8 — Honour caller character budgets on every content path (P2, S)

`max_chars_per_item` must bound the returned readable text for PDF and container
documents exactly as it does for HTML, at every detail level. Today the same request
returns 3,092 chars (3,000 honoured, `text_truncated: true`) under `standard` and
`diagnostic` but 12,092 under `compact`, because the compact projection bypasses the
document window and falls back to the PDF page budget. Successful PDF responses must also
carry `total_pages` and `pages_returned` so an agent can paginate without provoking an
error.

Acceptance: a PDF request with `max_chars_per_item: 3_000` returns at most 3,000 readable
chars plus framing at `compact`, `standard`, and `diagnostic`, sets `truncated: true`,
records the effective budget, and reports `total_pages` plus the selected page range; the
same budget rule holds for DOCX/XLSX items.

Tests: extend `tests/web_fetch_integration.rs` with a PDF fixture per detail level and a
proptest that `chars_returned` never exceeds `max_chars_per_item` plus the framing
allowance.

## Verification

```bash
make check
cargo check --locked --no-default-features
cargo check --locked --no-default-features --features pdf,browser,documents
cargo check --locked --features egress
cargo check --locked --all-features
cargo test --locked --features mock --test mcp_tools
cargo test --locked --features mock --test web_fetch_integration
cargo test --locked --all-features --test evidence_contract
cargo test --locked --all-features --test provider_capability_contract
make bench-check
```

Plus a manual limited live trial (not CI) recording per-tool payload chars at default
detail against the acceptance ceilings above, with the release binary and keyless
defaults only.

## Stop conditions

- Any change to tool inventory, aliases, argument acceptance, provider ids, default
  provider set, or trust framing: stop and split into a separate registered milestone.
- A second advertised-fingerprint change beyond WP3: stop and register it.
- WP1 requiring M039's invariant to move: stop; the answer is a representation change,
  not a budget change.
- WP2 changing retrieval behaviour (providers queried, ranking, RRF): out of scope.

## Closure evidence required

Per-WP acceptance numbers from a fixture-backed deterministic suite; the fingerprint
old/new pair; `docs/tool-matrix.md`, `architecture/mcp.md`, and `docs/config.md`
updated for the changed defaults; `CHANGELOG.md` entry; the manual trial record; and
the routes in `plans/registry.md` + roadmap status updated in the same closure commit.

## Appendix — trial findings outside this plan

Recorded here so they are not lost; each needs its own registered milestone.

1. **Trust/authority classification is wrong for official sources.** `blog.rust-lang.org`
   is `community_discussion` / `news_or_blog`; `releases.rs` and `github.com/<org>/<repo>/releases`
   are `authority: unknown`; all five results of a release question were `confidence: low`
   and `coverage_status: insufficient`; infoworld.com ranked above the official blog.
   Owner: `src/meta` quality/ranking. Fix: an official-domain registry and an
   `official_documentation` boost.
2. **Local path/filename-only matches are not actionable.** Such cards carry
   `snippet: "Local file: <path>"` with `selected_span: null` and no symbol, while
   content matches do carry spans and excerpts. A content-hinted local query also hit
   `local_search_truncated` at 512 files/1.5 s. Owner: `src/meta/local_backend.rs`.
3. **Browser process lifecycle.** A clean server shutdown in the second pass created no
   new Chrome processes, and every render logs "Browser was not closed manually, it will
   be killed automatically in the background", so teardown relies on the drop path rather
   than an explicit close. Thirty-four headless Chrome processes with
   PPID 1 were present before that pass and the count was unchanged after it, so this pass
   created none; forced termination of a server still orphans browsers. Owner: fetch/browser
   lifecycle; log hygiene is WP7.
4. **Duplicate warnings.** The same DuckDuckGo challenge warning appeared three times in
   one `repo_search` response, inflating payload and reader noise.
5. **Keyless DuckDuckGo now challenges.** `#challenge-form` on search pages, with partial
   results still returned by brave/yahoo. Provider health/cooldown behaviour is worth a
   dedicated look but is not a defect by itself.
6. **Coverage after the second pass.** Tested: PDF (public and loopback, content-type,
   `.pdf` extension, and `%PDF-` magic detection, page ranges, reversed and out-of-range
   ranges, `metadata_only`, `brief`, `compact`, `standard`, `diagnostic`); browser render,
   auto fallback, screenshot, invalid executable, loopback policy, and a 404 page;
   `+documents` (DOCX, XLSX, CSV); `+egress` (provider routing versus direct fetch); and
   `--all-features` build. Not tested: PDF OCR and layout (reported `deferred`), encrypted
   PDFs, PPTX and EPUB, headed `browser-login` profiles (needs an interactive session),
   HTTP transport, and a provider-credentialed run (keyless defaults only).
7. **PDF text keeps ligature splits.** Extracted text contains "identi ﬁ ed" and
   "speci ﬁ cation": the ﬁ ligature is a separate glyph and the join inserts a space, so a
   substring search for "identified" or "specification" fails. Owner: `src/fetch/pdf.rs`
   normalisation. P3, correctness of searchable text.
8. **Extraction completeness and browser status handling** are routed to M055; this
   appendix only records that the payload findings above were measured on the same
   responses.
