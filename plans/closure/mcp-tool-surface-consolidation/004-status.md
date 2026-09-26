# Tool-Surface Consolidation M004 — Result Projection Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/004-agent-result-projection-and-context-budget.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: M001 contract plus M003 structured capture; satisfied.

Soft dependency: M001 (hard vocabulary), M003 (structured results); both satisfied.

Implementation commit:

- `a9057125b0a9b3d51a2423fc2bbf411ddea8f715` — deterministic response-detail projection and context budgets

Closure candidate: `aab2e776b5177c3451713db4c737f1da8650318b` (no
projection change since `a905712`; M006/M007 add only harness and
guards).

## 1. Executive finding

M004 is complete. A central projection boundary
(`src/mcp/projection.rs`) implements `compact`/`standard`/`diagnostic`
(default diagnostic) over typed responses before serialization.
Compact preserves identity, trust, failure/absence distinction, and
next actions while cutting representative bytes; standard preserves
specialist semantics; diagnostic is passthrough. Canonical structured
results are captured before any display truncation.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| Central deterministic projection boundary | `src/mcp/projection.rs` (`project()`, `ResponseDetail`); no scattered `if response_detail` in tools | pass |
| Compact reduces bytes, preserves trust/coverage/failure/IDs/next actions | `compact_reduces_bytes_for_representative_payloads`; web ~37%, fetch ~49% representative savings | pass |
| Standard preserves specialist semantics | `standard_preserves_specialist_fields` (retrieval, routing summary, coverage) | pass |
| Diagnostic retains observability | `diagnostic_is_passthrough_for_all_tools` (9 tools) | pass |
| Structured results captured before truncation | Projection applies to typed values at MCP boundary; `response_detail` param defaults to diagnostic and round-trips | pass |
| CodeGG can store full content, show bounded projection | Handoff documented in `codegg-contract.md`/`codegg-integration.md`; bundle identity test below | pass |
| Compact cannot turn failure into negative evidence | `compact_preserves_failure_vs_absence_distinction`, `compact_keeps_partial_evidence_when_one_provider_fails` | pass |

Required-case coverage (`tests/mcp_projection.rs`, 14 tests):
all-providers-fail vs no-match, partial failure, injection markers,
conflicts (indicator vs metadata), applicability
affected/not-affected/unknown, fetch truncation/focus, batch partial
failure + budget exhaustion, local-trusted vs external-untrusted,
bundle identity across modes, byte-reduction thresholds, param
default/round-trip.

## 3. Production implementation evidence

- `src/mcp/projection.rs` (546 lines): compact keeps query identity,
  cards/groups, stable IDs, locators, trust + injection markers, 1
  excerpt per card, essential warnings, explicit failure/absence state
  (`providers_failed` + minimal `retrieval_status`), next
  actions/suggested fetches, conflict indicators; omits full
  routing/telemetry/document/link detail. Standard adds full
  retrieval, conflict, coverage, capability summaries. Diagnostic is
  passthrough. `provider_status` stays diagnostic-only;
  `build_evidence_bundle` is identity-preserving across modes.
- Context budget order enforced: fewer low-ranked cards, shorter
  excerpts, fewer diagnostics, focused spans, explicit truncation
  markers; never arbitrary JSON truncation before host storage.
- Schema budget 72k → 74k for `response_detail` (+~1k).
- Representative fixture savings: web ~37%, fetch ~49% in compact mode.

## 4. Verification executed

Against candidate `aab2e77`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test mcp_projection
cargo test --locked --features mock --test dispatch_fault_injection
cargo test --locked --features mock --test batch_fetch_retrieval
cargo test --locked --all-features --test evidence_contract
cargo test --locked --all-features
make bench-check
```

Outcomes:

- `mcp_projection`: 14 passed, 0 failed
- `dispatch_fault_injection` (mock): 32 passed
- `batch_fetch_retrieval` (mock): 13 passed
- `evidence_contract`: pass
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make bench-check`: pass

Before/after representative sizes are the web ~37% / fetch ~49%
compact savings above with per-case byte assertions in
`compact_reduces_bytes_for_representative_payloads`; full per-tool
tables live in the test fixtures and the M006 end-to-end byte report
(73037 definitions, 1614 instructions).

## 5. Invariant review

Stable IDs, provenance, trust, warnings, and partial/failure state
preserved in compact; security applicability, confidence, identifiers,
and KEV relevance never omitted; no LLM summarization inside
eggsearch; no per-vendor response shapes; host observability stays out
of prompts.

## 6. Failure and recovery review

Compact distinguishes retrieval failure (`has_failures`) from evidence
absence (`has_absences`); partial evidence retained with failing
provider listed; batch preserves per-item ok/error plus aggregate
budget exhaustion; truncation marked explicitly.

## 7. Migration and compatibility review

Default `diagnostic` preserves current observability for existing
callers; compact/standard are opt-in via `response_detail`. Bundle
content identical across modes.

## 8. Security review

Trust markers and injection warnings preserved in compact; no
projection path hides adversarial markers.

## 9. Documentation and operations

`architecture/mcp.md`, `architecture/codegg-contract.md`,
`docs/tool-matrix.md`, `docs/agent-workflows.md`,
`docs/codegg-integration.md`, README, AGENTS.md, and skills document
the three modes, budget order, and the store-full/show-bounded CodeGG
seam.

## 10. Residual findings

| Severity | Finding |
|---|---|
| None high/medium/low | No interpretation-critical omission identified; byte thresholds guard future growth. |

## 11. Roadmap disposition

M004 moves to closed with this record as controlling evidence.
M005 consumes the projection contract; M006/M007 already closed.

## 12. Registry updates

Covered in the same closure commit: M004 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/004-status.md`.
