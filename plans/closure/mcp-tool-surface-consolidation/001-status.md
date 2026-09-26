# Tool-Surface Consolidation M001 — Contract and Disclosure Model Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/001-contract-and-disclosure-model.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: `e0ea1e7` (phase 15 hygiene closure, pre-consolidation mainline).

Hard dependency: none (first in workstream; establishes vocabulary for M002-M007).

Implementation commit:

- `4b7e725887da2848546db6397bd8bc69bb0eff14` — canonical tool contract and disclosure model

Closure candidate: `aab2e776b5177c3451713db4c737f1da8650318b` (M007
closure; no production tool-definition delta from `4b7e725` for M001
gates — later M002-M005 narrow schemas and add output schemas around
the same contract descriptions, M006/M007 add only harness and guards).

## 1. Executive finding

M001 is complete. One authoritative registry (`src/mcp/tool_contract.rs`)
covers exactly the ten stable tools with concise selection-oriented
descriptions, purpose/use-when/not-for guidance, domain and advisory
disclosure hints, MCP annotations, discovery keywords, aliases, and
related/next-tool graphs. Server instructions are global rules only.
Contract tests fail on name drift, bad references, or context creep.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| All ten tool names remain registered and callable | `stable_tool_registration_count_and_names` + `registry_covers_exactly_registered_tools` (10/10) | pass |
| One authoritative registry consumed by server metadata/tests | `src/mcp/tool_contract.rs` (`ALL_CONTRACTS`, `lookup`, `tool_names`); `server.rs` descriptions match contract verbatim | pass |
| `provider_status` no longer a normal first research step | Diagnostic disclosure + description disclaimer + instructions rule; `diagnostic_tool_is_not_a_research_prerequisite` | pass |
| Server instructions no longer duplicate per-tool paragraphs | `server_instructions_are_global_rules_only` (1614 bytes, global rules, no minimum-call duplication) | pass |
| Tool annotations present where library supports | `annotations_match_contract_hints` (all read-only; search/fetch open-world, status/bundle closed) | pass |
| Contract tests fail on drift, bad references, oversized descriptions | `registry_names_unique_and_deterministically_ordered`, `related_and_next_tools_reference_known_tools_only`, `descriptions_stay_within_size_budget` (cap 300) | pass |
| CodeGG compatibility unchanged | Native wrapper names untouched; no wire change; `provider_workstream_regression` CodeGG deserialization green | pass |

## 3. Production implementation evidence

- `src/mcp/tool_contract.rs`: `ToolContract` (name, description,
  purpose, use-when, not-for, domain, disclosure, read-only,
  open-world, related/next, keywords, aliases), `ToolDomain` (7
  values), `ToolDisclosureHint` (Core/Deferred/Diagnostic),
  `annotations()`, `discovery_text()`, `is_known_tool()`, `lookup()`,
  `tool_names()`; `ALL_CONTRACTS` in alphabetical order (10 entries).
- `src/mcp/server.rs`: each `#[tool(description = ...)]` shortened to
  a selection-oriented sentence answering what/when/required-input/
  neighbor distinction; longest is `repo_search` at 186 chars against
  the 300 cap; `EGGSEARCH_INSTRUCTIONS` reduced to global rules
  (1614 bytes vs pre-consolidation 5275).
- Disclosure assignments: Core (`web_search`, `web_fetch`,
  `repo_search`); Deferred (`batch_fetch`, `repo_fetch`, `repo_map`,
  `security_search`, `research_search`, `build_evidence_bundle`);
  Diagnostic (`provider_status`).
- `tests/mcp_tool_contract.rs`: 14 tests (registry parity, ordering,
  references, budgets, keywords, diagnostic framing, annotations,
  instructions, disclosure metadata, discovery queries, known-tool
  checks, next-action sanitization, runtime validity, fingerprint).

## 4. Verification executed

Against candidate `aab2e77`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test docs_tool_names
cargo test --locked --all-features --test static_guards
cargo test --locked --features mock --test provider_workstream_regression
cargo test --locked --all-features
make bench-check
make docs-check
```

Outcomes:

- `mcp_tool_contract`: 14 passed, 0 failed
- `docs_tool_names`: pass (code-derived from `server.rs`)
- `static_guards` (tool/transport shape): pass
- `provider_workstream_regression` (mock): 7 passed, incl. CodeGG backward-compatible deserialization
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make bench-check`, `make docs-check`: pass

## 5. Invariant review

Ten capabilities preserved; names stable; no mega-tool; disclosure
advisory only, never policy-enforcing; authorization, network policy,
fetch safety, and provider routing untouched (still with their
architectural owners).

## 6. Failure and recovery review

Unknown `next_actions` targets ignored via `is_known_tool`; no widened
authority. Diagnostic tool carries no `next_tools` chain into normal
research flow.

## 7. Migration and compatibility review

No wire change; no new required arguments; serialized request/response
formats unchanged. CodeGG native wrappers resolve the same upstream
names. No capability hidden by deferred/diagnostic metadata.

## 8. Security review

No new I/O or authorization surface. Descriptions contain no
operational secrets; external content remains untrusted data per the
retained global instruction.

## 9. Documentation and operations

Reconciled `docs/tool-matrix.md`, `docs/agent-workflows.md`,
`AGENTS.md`, `architecture/mcp.md`, `architecture/codegg-contract.md`,
`docs/codegg-integration.md`, and skills so `provider_status` is
diagnostic-only and bootstrapping guidance matches the contract.
Narrative docs stay handwritten; names/disclosure categories are
test-validated.

## 10. Residual findings

| Severity | Finding |
|---|---|
| None high/medium/low | No regression identified; description/instruction budgets are enforced by M006 gates going forward. |

## 11. Roadmap disposition

M001 moves to closed with this record as controlling evidence.
M002-M007 build on this vocabulary; M006/M007 already closed.

## 12. Registry updates

Covered in the same closure commit: M001 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/001-status.md`.
