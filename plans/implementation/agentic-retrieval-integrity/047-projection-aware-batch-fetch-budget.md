# M047 — Projection-Aware Batch Fetch Budget and Web Structural Degradation

Status: active
Date: 2026-11-23
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M039)
Baseline: c75bfec9c3cc13a53b3b2c061a326d99b1cc2c38 (docs/user-facing-polish-20261123, clean; based on origin/main)
Source roadmap: §2 current-state gaps, §5 dependency graph, §6 M047
Decision authority: M039 and ADR-0016/M029 unified fetch contract. No new ADR unless preserving the public meaning of existing payload telemetry proves incompatible with projection-aware budgeting.

References: plans/implementation/agentic-retrieval-integrity/039-fetch-emitted-payload-budget.md; plans/closure/agentic-retrieval-integrity/039-status.md; architecture/fetch.md, Batch returned-character and emitted-payload budgets; src/mcp/tools/batch_fetch.rs; src/mcp/projection.rs.

## Objective

Make the batch fetch budget match the item payload the selected response-detail projection actually returns. For both web and repository items, optional duplicated document structure must degrade before requested text is clipped or an item is omitted. Keep the aggregate cap, deterministic input ordering, existing failure outcomes, and the unified fetch schema unchanged.

## Discovered gap and why M039 did not catch it

M039 closed on 10ed7ed08ac47d80e39ca676ea157ecf0728518c. Its structural ladder is entered only for repository items in src/mcp/tools/batch_fetch.rs; the web branch clips text and trims selected metadata but does not remove a large document. The batch is budgeted before project_with_budget applies response_detail in src/mcp/projection.rs. M039’s closure explicitly records this compact-projection over-count as a residual limitation.

In the 2026-11-23 live three-page fetch, NASA and CDC items succeeded, while the Python documentation item was omitted as batch_total_budget_exhausted with 23,331 characters remaining. Each URL succeeded when fetched individually. This outcome does not by itself establish that document structure was the only cause; it is a reproduction target, not a pre-judged fix. The code path establishes that web document structure is not degraded before clipping or omission and that compact projection may remove fields only after their size was charged.

## Scope

In scope: apply the same selected item projection used by the final batch response before charging each item against max_total_chars; extend structural-degradation-before-text to web items while preserving the repository ladder and each item’s locator, requested text, focus, and addressing data; keep payload_chars and total_payload_chars truthful for projected item payloads; preserve explicit degradation warnings and omission outcomes; add full/standard and compact-mode tests.

Out of scope: raising limits or changing defaults/max_items; adding pagination; changing direct web_fetch or repo_fetch budgets, browser/PDF behavior, cache policy, or workspace authorization; changing the tool inventory, item schema, stable IDs, trust/sanitization rules, or envelope-overhead policy.

## Invariants that cannot regress

- max_total_chars caps the sum of projected per-item response payloads in the existing character unit. Common envelope overhead remains excluded and documented.
- payload_chars and total_payload_chars use that same projected measure and remain deterministic. chars_returned retains its M036 readable-text meaning; truncated means requested text was clipped.
- Requested text is preserved while removable optional structure can make the item fit. Every structural drop remains visible in degraded_fields and the existing warning contract.
- A budget-omitted item remains ok: false with a null response and the M036 budget error; result order and continue_on_error behavior do not change.
- Projection is shared with the output path; do not maintain a second approximation that can drift. Existing offset/limit addressing, focus results, stable identifiers, trust markers, sanitization, deadlines, and SSRF controls remain unchanged.
- No schema/property or default change is required. Document any change from the current pre-projection telemetry to the projected caller-visible count; the value must remain a conservative count of what the caller receives.

## Expected production changes

- src/mcp/tools/batch_fetch.rs: projection-aware per-item accounting and web structural degradation before text clipping or omission.
- src/mcp/projection.rs: factor or expose the smallest shared item projection so budget calculation and final output use identical transformations.
- tests/batch_fetch_retrieval.rs and tests/mcp_projection.rs; extend web-fetch integration coverage where the behavior is externally visible.
- architecture/fetch.md, docs/features.md, and docs/tool-matrix.md if they state batch-budget behavior; update docs/test-inventory.md only if a suite is added or renamed; append CHANGELOG.md with implementation.

## Compatibility and migration

No argument, response field, or configuration key is added. Compact-mode items may remain successful where a pre-projection estimate previously clipped or omitted them; payload telemetry may decrease because it measures the selected projected payload rather than fields the caller will not receive. Full/diagnostic output is unchanged unless its budget is exceeded. Explain this behavior and measure in architecture/fetch.md. No stored data or migration.

## Dependencies

- Hard: M039 (closed), including M036’s failed-item outcome and readable-text semantics.
- Interface: ADR-0016 and M029 (closed unified fetch surface and existing projections).
- Soft: none.

## Ordered work packages

1. Reproduce the gap with deterministic web and repo fixtures in full/standard and compact modes. Include a large document whose requested text fits after structure is removed, plus a compact item that fits only after projection. Record current counts, markers, and outcomes.
2. Use one shared projection path for each batch item type and response-detail mode. Budget the same projected response the final output exposes; preserve current character-counting units and documented envelope exclusion. Do not measure different data than the output.
3. Apply optional-structure degradation before clipping text. Preserve M039’s repository order. For web items, remove large document structure before clipping or omission; retain locators and minimal addressing metadata required by the existing contract. Keep metadata trimming explicit and deterministic. If the irreducible item cannot fit, use M036’s failed omission outcome.
4. Add tests asserting web text survives when structure removal makes it fit; compact mode does not charge removed fields; full mode charges returned fields; payload telemetry matches projected item responses; aggregate item payload is within the cap; warnings, truncation markers, M036 omission, and addressed windows remain correct.
5. Update architecture/fetch.md first, then affected user-facing tool docs and CHANGELOG.md. Do not claim max_total_chars bounds the full serialized MCP envelope.

## Verification

```sh
cargo test --locked --all-features --test batch_fetch_retrieval
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test extract_fetch_contract
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

No test requires network or credentials. If a suite is added or renamed, synchronize docs/test-inventory.md, architecture/testing.md, and skills/eggsearch-dev/SKILL.md.

## Acceptance and stop conditions

- For every response-detail mode, total_payload_chars equals the sum of per-item payload_chars after the same projection used for output and does not exceed max_total_chars (common envelope overhead remains outside the documented bound).
- A web item whose requested text fits after optional document structure is removed remains ok: true; it is not clipped or omitted first.
- Compact mode does not charge fields it removes; full/diagnostic mode charges fields it returns. Output without budget pressure is unchanged.
- Text, focus, offsets, stable IDs, sanitization/trust fields, warning taxonomy, and M036/M039 failure semantics remain covered; broad gates pass.
- Stop and record a separate decision if truthful post-projection counts require a breaking wire change, shared projection cannot preserve final-output parity, or the budget can only be met by removing required addressing/trust metadata. Do not silently raise a cap or change a default.

## Required closure evidence

Create plans/closure/agentic-retrieval-integrity/047-status.md with the exact candidate SHA; before/after web and repo results in full and compact modes; a requirement-to-evidence matrix; projected item-size calculations and envelope-exclusion caveat; per-item and aggregate budget behavior; warning/omission/addressing compatibility; focused and broad test outcomes; hosted qualification; and remaining limitations.
