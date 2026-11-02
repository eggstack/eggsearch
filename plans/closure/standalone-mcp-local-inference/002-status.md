# Standalone MCP Local-Inference M002 — Brief Projection Closure

Status: closed

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/002-brief-evidence-and-output-budgets.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Repository baseline reviewed: M001 closure commit `d983fa5` and implementation
candidate `02a8af6fe4f87971c7daca7d1fca20c087b328ec`.

Implementation candidate: `02a8af6fe4f87971c7daca7d1fca20c087b328ec`.
M001 closure: `d983fa5` (`plans/closure/standalone-mcp-local-inference/001-status.md`).

## 1. Executive finding

M002 is closed. The additive `brief` result mode defaults only when the
operator selects the compact profile and omits `response_detail`; explicit
per-call values still win, and the full profile remains diagnostic by default.
Brief responses use deterministic field-aware reduction and a configurable
serialized-character ceiling. Status, failure-vs-absence, trust and source
identity remain visible; oversized applicability and provider-failure arrays
retain summary counts, and omitted batch failures retain their member indexes.

Nontrivial fixtures exceed the plan's 30% reduction criterion. Sparse task
corpus results are separately reported at 11.8% reduction and are not hidden
behind the richer fixtures. No LM Studio model-token or task-quality claim is
made here; that is M003's live consumer gate.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Brief is additive and profile-aware | `src/mcp/projection.rs`, config and tool tests | pass | Compact omitted detail => brief; full => diagnostic; explicit detail wins |
| Budget is bounded and configurable | `tests/mcp_projection.rs`, config validation | pass | Default 8,000 characters; accepted range 512–32,000 |
| Representative nontrivial outputs save at least 30% | `docs/retrieval-quality-context-efficiency.md` | pass | Web search 48.0%; fetch 74.6%; synthetic repo/research stress fixtures 93.8% |
| Failure, absence and partial status survive | Projection fixtures and MCP/domain suites | pass | Provider failure summaries retain total and error classes |
| Security applicability remains truthful under pressure | 40-entry applicability stress fixture | pass | State totals retained: 14 affected, 13 not affected, 13 unknown; output <=512 chars |
| Batch failure members are not silently erased | 20-member failure stress fixture | pass | Each failure remains as a record or a listed `failed_member_indices` value under 512 chars |
| Stable IDs, trust markers, truncation and PDF coordinates survive | `tests/mcp_projection.rs` | pass | Explicit regression coverage |
| Evidence bundle stays identity-preserving | `tests/mcp_projection.rs` | pass | Brief does not trim canonical bundle identity |
| Sparse corpus is reported without an artificial target | Context-efficiency report | pass | 21,221 diagnostic vs 18,715 brief bytes (11.8% smaller); same 14 retrieved relevant documents |

## 3. Production implementation evidence

`ResponseDetail::Brief` is parsed by the shared MCP projection layer. It
applies existing compact safety reductions, an explicit field allowlist,
UTF-8-safe content clipping, deterministic record omission and
`returned_count`/`omitted_count`/`output_truncated` metadata. If a security
applicability list cannot fit, `applicability_summary` preserves per-state
counts and exposes omitted entries. Provider failures retain a total and
error-class counts; batch failures retain indexes when full failure records
must be omitted. The tool handlers use the configured brief budget for both
structured content and the text fallback. Diagnostic remains available as an
explicit override. Evidence-bundle identity and the diagnostic-only provider
status contract are retained.

Measured fixtures (serialized bytes):

| Fixture | Diagnostic | Brief | Reduction |
|---|---:|---:|---:|
| Rich web search with provider failure | 2,728 | 1,418 | 48.0% |
| Web fetch with 2,000-character text and a link | 4,292 | 1,090 | 74.6% |
| Repo grouped stress fixture, deliberately redundant | 10,846 | 669 | 93.8% |
| Research grouped stress fixture, deliberately redundant | 10,846 | 669 | 93.8% |
| Sparse 15-task deterministic trace | 21,221 | 18,715 | 11.8% |

The repo/research figures are synthetic stress fixtures, not production
averages. Byte and character costs are not model-token costs.

## 4. Verification executed

- `rtk make check` — pass; formatting, Clippy, no-default-features check,
  all-features tests (3,486), docs, hygiene, dependency policy and packaging
  contract checks passed.
- `rtk make docs-check` — pass after implementation and M001 closure update.
- `rtk make bench-check` — pass; optimized Criterion harness compiled with
  `--no-run` (11m32s on this host).
- `cargo test --locked --all-features --test mcp_projection` — pass
  (21 tests, including oversized applicability/provider/batch cases).
- `cargo test --locked --all-features --test mcp_http` — pass (12 tests).
- `cargo test --locked --all-features --test mcp_2026_protocol` — pass
  (14 tests).
- `cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture`
  — pass (5 tests; both profiles 43/43 top-1 and recall@3, MRR 1.000).
- `rtk git diff --check` — pass.

## 5. Invariant review

Projection operates only at the MCP presentation boundary. It does not change
upstream retrieval, candidate count, ranking, provider selection, cache,
browser/PDF extraction, security decisions, stable identity, or execution
budgets. No inference, summarization, persistent result cache, or follow-up
fetch is introduced. Full/diagnostic remains the unmodified configuration
default.

## 6. Failure and recovery review

Budget fitting omits complete successful records before critical state. If
large failure or applicability collections still exceed the budget, the
summaries and omitted-index/count signals keep the result bounded while
retaining aggregate truth. The typed bundle path remains complete. Explicit
diagnostic provides the complete bounded diagnostic payload when requested.

## 7. Migration and compatibility review

No migration is required. `brief` is additive; existing `compact`, `standard`
and `diagnostic` modes remain available. Full-profile default behavior remains
diagnostic. Compact is opt-in through server configuration; explicit call
values override the profile default.

## 8. Security review

Untrusted framing, trust markers, source locators, failure state and security
applicability are retained in brief results. Large applicability lists retain
all status totals even when individual records are omitted. No authorization,
SSRF, provider-routing, local-filesystem, or egress policy changed.

## 9. Documentation and operations

`architecture/mcp.md`, `docs/config.md`, `docs/test-inventory.md`,
`docs/retrieval-quality-context-efficiency.md`, and `docs/lm-studio.md` explain
the profile default, brief budget, semantics, measurements, and live-client
limits. The ordinary rollback is to set `schema_profile = "full"` and restart
the server process.

## 10. Unresolved findings

- Low, operational: sparse result sets show less than the 30% reduction target;
  this is reported as observed and no universal reduction is claimed.
- Low, operational: LM Studio tokenizer, host-injection and task-quality
  behavior remain unmeasured; M003 is blocked pending live consumer evidence.
- No M002 correctness or security finding remains open.

## 11. Roadmap disposition

M001 and M002 are closed. M003 implementation dependencies are satisfied, but
M003 remains blocked until live LM Studio model-turn, context-cost,
compatibility and task-quality evidence is obtained. The offline measurements
in this record do not discharge that condition.

## 12. Registry updates

In the same closure commit, M002 is marked closed, the roadmap records
M001-M002 closed and M003 blocked on live client evidence, and
`plans/registry.md` points to this record. A separate M003 status record names
the remaining operational gate.
