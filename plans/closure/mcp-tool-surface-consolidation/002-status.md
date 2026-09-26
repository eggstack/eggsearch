# Tool-Surface Consolidation M002 — Schema Slimming Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/002-agent-facing-schema-slimming.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: `4b7e725` (M001 contract registry).

Hard dependency: M001 metadata stable; satisfied.

Implementation commit:

- `b1e6ea76ba2304847e59dc307b4c7127b0b74f2c` — slim agent-facing schemas with canonical goal/sources translators

Closure candidate: `aab2e776b5177c3451713db4c737f1da8650318b` (later
M003 adds output schemas and M004 adds `response_detail`, both
accounted in the current 74000 total budget; ordinary input schemas
unchanged since `b1e6ea7`).

## 1. Executive finding

M002 is complete. Ordinary advertised schemas are materially smaller
with all capabilities reachable: `repo_search` exposes `goal`/`sources`
instead of `profile`/`mode`/broad `workflow`/source-boolean forest;
`research_search`/`security_search` expose `goal`/`include`;
`web_search` hides `providers`/`timeout_ms`. Legacy fields remain
accepted by the runtime compatibility superset with explicit conflict
rules and repairable errors. The four-search reduction exceeds the 30%
target.

## 2. Requirement-to-evidence matrix

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| All ten tools and backend capabilities callable | All suites green; legacy-field deserialization tests (repo/web/research/security) | pass |
| Four search schemas materially smaller (≥30%) | `search_four_schema_bytes_stay_slim`: pre 17723 → post 9789 bytes (-44.8%, budget 12500) | pass |
| `repo_search` no longer reasons over profile/mode/workflow/booleans | `repo_search_advertises_goal_and_sources` (goal+sources advertised; 14 legacy keys hidden) | pass |
| Research/security use consistent task vocabulary | `research_search_advertises_goal_and_include`, `security_search_advertises_goal_and_include` (translation layer, no enum merge) | pass |
| CodeGG compatibility fixtures pass | `provider_workstream_regression` + `provider_request_contract` green | pass |
| Legacy clients supported with documented window | Runtime structs accept hidden fields; `legacy_*_still_deserialize` tests; operator path documented | pass |
| Conflicting/invalid controls give repair guidance | `repo_goal_conflict_reports_repair`, `repo_unknown_goal_enumerates_canonical_values`, `repo_sources_conflict_reports_repair`, research/security conflict tests | pass |

## 3. Production implementation evidence

- `src/mcp/tools/canonical.rs`: shared translators
  (`resolve_repo_semantics`, `resolve_repo_sources`,
  `resolve_research_workflow`, `resolve_research_includes`,
  `resolve_security_workflow`, `resolve_security_includes`) with
  precedence (canonical wins only when legacy absent/equivalent) and
  actionable repair errors enumerating canonical values.
- Ordinary schemas: `web_search` ≤13 props (no `providers`/
  `timeout_ms`); `repo_search` ≤23 props (`goal`+`sources`, no
  `profile`/`mode`/`workflow`/source booleans/provider/timeout);
  `research_search` ≤15, `security_search` ≤18 (both `goal`+`include`,
  legacy booleans/`workflow`/provider/timeout hidden).
- Before/after (from implementation commit, locked by tests):
  search-4 17723 → 9789 bytes (-44.8%); total input-only 74694 →
  66760 bytes. Current total budget 74000 covers M003 output schemas
  (+~4k compact envelopes) and M004 `response_detail` (+~1k).
- `tests/mcp_schema_slimming.rs`: 16 tests (byte budgets, property
  counts, hidden/advertised keys per tool, legacy deserialization ×4,
  conflict/repair ×5).

## 4. Verification executed

Against candidate `aab2e77`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test mcp_schema_slimming
cargo test --locked --features mock --test provider_workstream_regression
cargo test --locked --features mock --test provider_request_contract
cargo test --locked --all-features
make docs-check
make bench-check
```

Outcomes:

- `mcp_schema_slimming`: 16 passed, 0 failed
- `provider_workstream_regression` (mock): 7 passed
- `provider_request_contract` (mock): 21 passed
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make docs-check`, `make bench-check`: pass

## 5. Invariant review

No tool removed or merged; provider selection still possible for
advanced callers via accepted-but-hidden fields; safety semantics
unchanged; no mega-tool union schema.

## 6. Failure and recovery review

Equivalent canonical/legacy combinations accepted; incompatible
combinations return `conflicting goal`-style errors with bounded
`Repair` hints listing accepted values. Unknown values enumerate
canonical options without dumping implementation aliases.

## 7. Migration and compatibility review

Additive canonical fields; legacy fields hidden from ordinary schema
but deserialized and translated to the same internal representation.
Historical CodeGG request fixtures still deserialize and execute
equivalently. No silent reinterpretation of conflicting fields.

## 8. Security review

No policy change; hidden infrastructure controls remain enforced
server-side when supplied. No new untrusted-input surface.

## 9. Documentation and operations

Mapping table for repo/research/security vocabulary in plan and
`canonical.rs` error text; `docs/agent-workflows.md`,
`docs/tool-matrix.md`, `docs/codegg-integration.md`, and skills updated
to canonical `goal`/`include` examples with legacy path documented for
advanced callers.

## 10. Residual findings

| Severity | Finding |
|---|---|
| Low, accepted | Total definition budget grew 66760 → 73037 after M003/M004 by design (output schemas + `response_detail`); input-schema slimming itself is preserved and separately budgeted (search-4 ≤12500). |
| None high/medium | No capability loss or compat break identified. |

## 11. Roadmap disposition

M002 moves to closed with this record as controlling evidence.
M003-M007 build on the canonical seams; M006/M007 already closed.

## 12. Registry updates

Covered in the same closure commit: M002 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/002-status.md`.
