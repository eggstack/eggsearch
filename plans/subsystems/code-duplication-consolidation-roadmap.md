# Code Duplication Consolidation Roadmap

Status: closed (M001–M008 closed 2026-11-22; evidence in `plans/closure/code-duplication-consolidation/`)

Long-term references:

- `plans/000-long-term-specification.md` §1, §2, §7
- `plans/001-terminology-and-domain-model.md` §1, §3
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md` §3 (classification), §5 (sizing), §12 (subsystem boundary
  "maintenance, architecture consolidation")

Related ADRs:

- `plans/adrs/ADR-0016-unified-fetch-discovery-and-legacy-alias-compatibility.md`
- `plans/adrs/ADR-0017-fetch-path-challenge-resolution.md` §4 (one orchestration module)
- `plans/adrs/ADR-0022-nine-tool-surface-after-repo-fetch-retirement.md`
- `plans/adrs/ADR-0023-one-repository-target-field-set.md` (accepted; M007 closed under this decision)

Supersedes no roadmap. The closed maintenance, transport-consolidation, and tool-surface
workstreams are historical evidence and are not reopened. This roadmap owns a new, bounded
consolidation pass over duplication that the 2026-11-22 repository review measured and that no
existing roadmap covers.

## 1. Purpose and ownership boundary

Owns mechanical and structural duplication that raises the cost of ordinary change: shared
integration-test boilerplate, repeated tool-argument construction, the provider capability
declaration block, the forge engine request skeleton and its duplicated unit tests, the
`SearchEngine` trait implementation glue, the duplicated fetch-escalation orchestration, the
four request validators, and the four repository-target field sets.

Does not own retrieval semantics, ranking, provider selection, fetch safety policy, browser
lifecycle policy, schema economy targets, or the domain planners. Every milestone is
behaviour-preserving unless it says otherwise; where a contract is touched (M007), the change is
to Rust composition, not to the JSON contract, and it is ADR-gated.

## 2. Work classification

### Invariants

- No caller-visible behaviour change: same MCP tool names, argument names, accepted values,
  response fields, error codes, and messages.
- Same trust model: sanitization, trust markers, SSRF policy, bounded reads, content-derived
  stable IDs, and the `core <- meta <- mcp <- commands` dependency flow.
- `src/fetch/**` stays independent of `src/meta/**`; `challenge_replay` stays above both.
- Keyless HTML scraping keeps its four-outcome contract; no parser is rewritten.
- No test may require network. `make check` and `make docs-check` stay the gates.
- File-size ratchets are not raised; new modules are added rather than growing capped ones.

### Capabilities

- A contributor adds one repository-target field, or one tool argument, in one place and the
  compiler or a test names every other site that must follow.
- One table-driven conformance suite proves the shared invariants of the API engines, so a new
  engine inherits the coverage instead of copying it.
- One escalation implementation serves the MCP fetch surfaces and the CLI.

### Infrastructure

- `tests/support/mod.rs` as the single integration-test harness.
- `Default` on the tool argument structs that are built field by field in tests.
- A shared bounded JSON transport for the forge engines.
- A shared capability declaration form for provider descriptors.
- Shared request-validation helpers.
- A macro for the `SearchEngine` forwarding impls.

### Polish

- Measured line reduction and removal of copy-paste drift. Not a capability claim.

## 3. Non-goals

- Flattening the repo, research, and security planners or groupers. The 2026-11-22 review
  normalized workflow names and measured 1,207–1,352 differing lines per pair; they are not
  redundant, and ADR-0001 and the canonical specification keep them separate.
- Rewriting per-engine HTML row parsing. The four-outcome contract is deliberately per engine.
- Changing the advertised tool inventory, schema economy budgets, preset membership, or alias
  dispatch.
- Unifying the four search request types or their two error conventions (M006 only shares the
  bounds checks).
- Refactoring the registry engines' `resolution_to_result` builders in this workstream. Their
  bodies are structurally similar but not identical; the review ranked them last and no plan
  here owns them. A future review may propose them.
- Adding a dependency, a feature flag, a configuration key, or a network requirement in tests.

## 4. Baseline state

Measured 2026-11-22 on `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on
`feat/auto-20261117-044c4885`, by scanning 387 Rust files under `src/` and `tests/` with an
8-line normalized-window clone detector, then reading every match that the summary below relies
on. Struct-initializer noise was filtered so the percentages describe logic, not `None` fields.

| Fact | Value |
|---|---|
| Cross-file clone share, `src/` | 3,701 of 121,723 normalized lines (3.0%) |
| Cross-file clone share, `tests/` | 3,491 of 65,216 normalized lines (5.4%) |
| Test-module share of `src/` | 62,527 of 185,007 lines (33.8%) |
| Test-module share of `src/meta/engines/` | 13,267 of 27,787 lines (47.7%) |
| Forge engine family | 9 files, 7,493 lines, 75–82% test module per file |
| Byte-identical test helpers | 12 helpers in 8 suites, 173 lines per copy (~1,211 redundant) |
| Field-by-field argument literals | 116 `WebFetchArgs`, 87 `RepoFetchArgs` |
| Mechanical `SearchEngine` impls | 39 impls, ~853 lines (`src/meta/engines/mod.rs:358`–`1210`) |
| Inline capability literals | 39 × 24 fields in `built_in_provider_descriptor` |
| Repository-target shapes | 4 structs of 18–20 fields each |
| Request validators | 4 impls across 2 error types |
| Escalation copies | MCP at `src/mcp/tools/web_fetch_escalation.rs:78`/`:93`; CLI at `src/commands/fetch.rs:281`/`:299` |

The concentration, not the aggregate, is the finding. Duplication clusters in the test harness,
the forge engine family, the provider descriptor block, and the fetch surfaces. The review
ranked the blast radius from parallel field sets above the raw line count: a single new argument
currently touches up to four structs, three conversions, the schema pins, and the documented
tables.

Existing guards that the work must keep green: `tests/mcp_tool_contract.rs` (fingerprint and
byte pins), `tests/mcp_schema_slimming.rs` (per-tool budgets), `tests/provider_capability_contract.rs`
(capability claims), `tests/engine_parse_conformance.rs` (HTML four-outcome fidelity),
`tests/static_guards.rs` (layering, hygiene, planning), and `tests/tool_inventory_parity.rs`
(one inventory, every projection).

## 5. Target architecture

One source per fact, with the compiler or a table test naming every projection:

```text
tests/support/mod.rs        one integration-test harness
core::provider              one capability declaration form (none() + overrides)
meta::engines::api          one bounded JSON transport for API engines
tests/engine_api_conformance one table for the shared engine invariants
fetch::browser::escalation  one escalation orchestration with a caller-supplied render
core::validation            one set of query/limit bounds helpers
core::repo_fetch::RepoTarget one repository-locator field set (ADR-0023)
engines::impl_engine!       one forwarding implementation per engine
```

Nothing in this list is a new abstraction over domain behaviour. Each item replaces a copy that
already exists, and each is introduced behind an existing test or with one added first.

## 6. Dependency graph

```text
M001 test support + defaults (independent, test-only)
M002 provider capability compaction (independent)
M006 core validation helpers (independent)

M003 engine API conformance harness  --hard-->  M004 shared engine JSON transport
                                                    |
                                                    +--soft--> M008 SearchEngine impl macro

M005 shared fetch escalation seam (independent; interface ADR-0017 §4, M046 closure debt)

ADR-0023 acceptance --hard--> M007 one repository-target field set (accepted; closed)
```

Ordering is by risk and leverage, not by value: M001 and M002 are cheap and mechanical; M003 is
the safety net for M004; M005, M006, and M008 are independent; M007 waits for its ADR.

## 7. Milestones

### M001 — Test support harness and default-constructed tool arguments

Class: infrastructure. Objective: one integration-test harness and `Default` on the three tool
argument structs that tests build field by field, so a new argument stops touching dozens of
sites. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/001-test-support-harness-and-default-arguments.md`.
Dependencies: none. Exit: eight suites compile against one helper module, the field-by-field
literal count drops materially, and no production behaviour or schema changes.

### M002 — Provider capability declaration compaction

Class: polish. Objective: express each provider's capability set as `none()` plus the flags it
actually enables, pinned by a full-matrix test. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/002-provider-capability-declaration-compaction.md`.
Dependencies: none. Exit: every `KNOWN_PROVIDER_IDS` descriptor is identical to the pinned
matrix, and the declaration block shrinks by hundreds of lines.

### M003 — Engine API conformance harness

Class: infrastructure. Objective: one table-driven suite covering the invariants the nine forge
API engines currently re-test by copy, then delete the copies. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/003-engine-api-conformance-harness.md`.
Dependencies: none. Exit: the shared invariants are asserted once per engine from one table and
the duplicated engine tests are gone, with no production change.

### M004 — Shared engine JSON transport

Class: infrastructure. Objective: one bounded JSON GET helper plus shared small helpers for the
forge engines, so each engine owns only its request parameters and its `convert`. Implementation
plan: `plans/archive/implementation/code-duplication-consolidation/004-shared-engine-json-transport.md`.
Dependencies: hard M003 (the conformance suite is the safety net). Exit: the duplicated
send/status/parse block is gone, error mapping is unchanged, and M003 stays green.

### M005 — Shared fetch escalation seam

Class: infrastructure. Objective: one orchestration for the gated-status escalation used by the
MCP fetch surfaces and the CLI, with the render supplied as a callback so no layer inversion is
introduced. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/005-shared-fetch-escalation-seam.md`.
Dependencies: none; interface ADR-0017 §4. Exit: one implementation of the deadline floor,
timeout, return rule, and completion accounting; the CLI keeps its surface and the MCP keeps its
permits, profiles, and screenshots.

### M006 — Core request validation helpers

Class: polish. Objective: one set of bounds helpers behind the four request validators, keeping
each validator's error type and messages. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/006-core-request-validation-helpers.md`.
Dependencies: none. Exit: four validators delegate the shared checks; every existing validator
test and error string is unchanged.

### M007 — One repository-target field set

Class: compatibility + infrastructure. Objective: one `RepoTarget` field set composed into the
four repository-target shapes, one locator-to-request conversion, and a parity test, under
ADR-0023. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/007-one-repository-target-field-set.md`.
Dependencies: hard ADR-0023 acceptance. Exit: one field list, unchanged JSON contract, measured
schema bytes inside budget, and `repo_fetch` still answers identically as an alias.

### M008 — SearchEngine implementation macro

Class: polish. Objective: replace 39 forwarding `impl SearchEngine` blocks with one macro and
per-engine call expressions. Implementation plan:
`plans/archive/implementation/code-duplication-consolidation/008-searchengine-implementation-macro.md`.
Dependencies: soft M004 (same directory, import churn). Exit: the trait scaffolding exists once,
every engine name and behaviour is unchanged, and the engine suite count is unchanged.

## 8. Cross-cutting requirements

Storage: none. Protocol: no MCP tool list, argument, accepted value, or response field changes;
M007 changes Rust composition only. Security: no new outbound path, no relaxation of SSRF,
sanitization, bounds, or trust marking; test-only code gains no production reachability.
Failure semantics: every milestone preserves the current timeout, retry, and error-code
behaviour; M004 and M005 must prove it with the existing suites rather than new assertions of a
different contract. Observability: no new counters or logs; M005 keeps the existing completion
counter semantics. Documentation: each milestone updates the architecture section that owns the
changed seam and `docs/test-inventory.md` when a suite is added or its contents change.

## 9. Verification strategy

Per milestone: the focused suites named in its plan, then `make check` and `make docs-check`
on the exact candidate. Test-only milestones (M001, M003) additionally prove that the deleted
copies are gone by grep and that the new harness fails when a covered assertion is injected.
Refactors (M002, M004, M005, M006, M008) prove behaviour preservation by running the pre-existing
suites unchanged; the only new tests are equivalence pins and guards. M007 records schema bytes
and a live alias probe. No milestone may claim closure from compilation or from a subset gate.

## 10. Risks and decision points

- **ADR-0023 was accepted on 2026-11-22.** M007 used flattening within the existing `fetch`
  schema budget; the before/after measurements and parity evidence are recorded in its closure
  record. The macro fallback was not needed.
- **M003 can weaken coverage if it deletes the wrong tests.** The table must cover the union of
  the invariants, not the intersection; engine-specific conversion and metadata assertions stay
  in the engine modules. The plan requires the deleted-case inventory in the closure record.
- **M004 touches error construction.** `EngineError` variants carry the engine name and the
  status/reason text; the helper must take the name as a parameter and preserve each message.
  A conformance case asserts variant and text, not just the variant.
- **M005 has a layering trap.** `run_browser_fetch` owns MCP permits, profiles, and the mock
  override and must stay in `mcp`; the shared helper takes it as a callback. If that cannot
  preserve the mock path, the milestone stops and records the residual duplication again rather
  than inverting `fetch -> mcp`.
- **M001 is test-only but touches every suite.** A helper that a suite does not use will trip
  `dead_code`; the module needs per-helper feature gates and an explicit allow. The plan names
  this so an agent does not "fix" it by deleting helpers another suite needs.
- **M008 can obscure overrides.** Engines that override `search_batch`, `supports_role`, or
  advisory methods keep hand-written impls; the macro covers only the forwarding shape.

## 11. Completion definition

Closed when M001–M006 and M008 each have accepted closure evidence on an exact candidate, and
M007 is either closed under an accepted ADR-0023 or explicitly deferred with the ADR rejected
and the four shapes documented as the accepted status quo. Completion is not a line-count
target; it is the absence of a second place to edit for the fields, helpers, and seams named in
§5.

## 12. Milestone status

| Milestone | Class | Status | Implementation plan | Blockers |
|---|---|---|---|---|
| M001 | infrastructure | closed | `plans/archive/implementation/code-duplication-consolidation/001-test-support-harness-and-default-arguments.md` | Closed; `plans/closure/code-duplication-consolidation/001-status.md` |
| M002 | polish | closed | `plans/archive/implementation/code-duplication-consolidation/002-provider-capability-declaration-compaction.md` | Closed; `plans/closure/code-duplication-consolidation/002-status.md` |
| M003 | infrastructure | closed | `plans/archive/implementation/code-duplication-consolidation/003-engine-api-conformance-harness.md` | Closed; `plans/closure/code-duplication-consolidation/003-status.md` |
| M004 | infrastructure | closed | `plans/archive/implementation/code-duplication-consolidation/004-shared-engine-json-transport.md` | Closed; `plans/closure/code-duplication-consolidation/004-status.md` |
| M005 | infrastructure | closed | `plans/archive/implementation/code-duplication-consolidation/005-shared-fetch-escalation-seam.md` | Closed; `plans/closure/code-duplication-consolidation/005-status.md` |
| M006 | polish | closed | `plans/archive/implementation/code-duplication-consolidation/006-core-request-validation-helpers.md` | Closed; `plans/closure/code-duplication-consolidation/006-status.md` |
| M007 | compatibility | closed | `plans/archive/implementation/code-duplication-consolidation/007-one-repository-target-field-set.md` | Closed; `plans/closure/code-duplication-consolidation/007-status.md` |
| M008 | polish | closed | `plans/archive/implementation/code-duplication-consolidation/008-searchengine-implementation-macro.md` | Closed; `plans/closure/code-duplication-consolidation/008-status.md` |
