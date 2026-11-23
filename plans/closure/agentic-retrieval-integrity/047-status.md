# M047 Projection-Aware Batch Fetch Budget and Web Structural Degradation — Closure Evidence

Status: closed

Source plan: plans/implementation/agentic-retrieval-integrity/047-projection-aware-batch-fetch-budget.md

Roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md M047

Decision authority: M039 and the existing M036/M029 batch-fetch and projection contracts. No new ADR, tool, argument, response field, default, stable ID, or dependency.

## Exact code candidate

Implementation candidate 9528d4131591392c0d9f6ec5f0422903e4b7815b on docs/user-facing-polish-20261123, based on c75bfec9c3cc13a53b3b2c061a326d99b1cc2c38. Candidate tree: ee8e56e747c92da196f419d763d14439a4ce97b1. Local commands ran against the content-identical pre-commit tree; the implementation commit is clean. This closure commit will carry the closure records and synchronized plan status only; it changes no source.

Hosted CI run 45127251813 succeeded on this exact SHA via workflow dispatch: Linux `ci`, Windows portability, and macOS canonical/filesystem portability all passed. The Node.js 20 deprecation annotation was non-blocking. Run: https://github.com/eggstack/eggsearch/actions/runs/45127251813.

## Defect and change

M039 charged a pre-projection item payload. Standard/compact/brief projections could remove content after that charge, and the structural degradation path was repository-specific. A web item could therefore be clipped or omitted while removable document structure was still present. Brief mode also projected the batch envelope only after initial budget/item selection, so early enforcement could skip later items before the complete Brief projection was known.

The implementation now uses the shared per-item projection for both measurement and final output, degrades returned optional structure before requested text, and refreshes item/aggregate payload telemetry after projection. Diagnostic, standard, and compact modes apply the aggregate cap during ordered processing; Brief mode waits until the complete Brief envelope projection is available, then enforces the cap against retained projected item responses. The Brief helper updates the projected JSON fields directly because Brief projection intentionally omits the row index.

## Requirement-to-evidence mapping

| Requirement | Evidence |
|---|---|
| Count the same projected per-item response the caller receives; preserve character-count units | tests/mcp_projection.rs::batch_payload_budget_fields_track_projected_responses checks Diagnostic, Standard, Compact, and Brief: each payload_chars equals the serialized projected response, and total_payload_chars is their sum. batch_web_payload_budget_counts_only_projected_fields confirms Compact does not charge a projected-away document. |
| Degrade web structure before clipping or omission | tests/batch_fetch_retrieval.rs::batch_web_budget_uses_projected_payload_and_drops_structure_before_text: Standard’s full projected document exceeds the computed cap, while removing document.blocks/document.chunks leaves room; the result stays successful, preserves requested text, is not text-truncated, reports both degradation markers, and reports the exact response-size count within the cap. |
| Compact projection is charged after its presentation fields are removed | The same web regression chooses a cap above the Compact serialized projection but below the original payload. The item remains successful with unchanged text, no document, no false budget-degradation markers, and exact payload telemetry within the cap. |
| Preserve repository behavior and M036 omission outcomes | batch_repo_degradation_drops_structure_before_text preserves a non-empty text prefix, drops lines before text clipping under a 25,000-character cap, and checks item/aggregate counts and both warning forms. batch_repo_metadata_budget_omission_is_a_failed_item checks an irreducible item remains ok: false, has null response and zero readable/payload characters, and updates failure telemetry. |
| Enforce only after the complete Brief projection; preserve deterministic fetch/item selection | brief_batch_budget_waits_for_final_projection uses two local endpoints and a 512-character aggregate cap; both requests are made, results remain, total projected payload is at most 512, and no result is misreported as “was skipped.” |
| Preserve addressed windows, partial failures, focus, truncation, and aggregate behavior | Existing mcp_projection addressed-window and batch-failure tests, plus batch_total_payload_chars_bounds_three_item_batch, remain green in the full gate. chars_returned remains the M036 readable-text measure; truncated still denotes requested text clipping. |

## Budget and compatibility evidence

payload_chars is the Unicode-scalar count of serde_json::to_string(projected_response); total_payload_chars sums those item counts. The cap excludes common envelope overhead: row wrappers, the results array, aggregate counts/telemetry, warnings, addressing fields, and projection stamps. This is not a serialized-envelope, byte, or token cap. These units and exclusions are documented in architecture/fetch.md and docs/features.md.

Before M047, Compact/Brief fields removed by projection could still be included in the pre-projection charge, and web document structure was not in the degradation ladder. After M047, tests calculate the projected web sizes from the fixture itself and assert that the response fits the cap without sacrificing requested text. For repository responses, the regression asserts the actual returned text and dropped-field marker under the cap; the projection suite checks exact counts for all four detail modes. The historical three-page live batch is retained as motivation, not as proof that document structure was its sole cause; no live network probe was run for this closure.

No tool/schema/default/identity, locator-safety, trust, sanitization, deadline, SSRF, or migration behavior changed. Existing budget warning taxonomy and omission semantics remain intact.

## Verification

- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test batch_fetch_retrieval: 29 passed.
- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_projection: 36 passed.
- RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib fetch::render::blocks: 34 passed (shared M048 rendering regression suite).
- RUSTUP_TOOLCHAIN=1.89.0 make check: passed, including formatting, Clippy, no-default-features check, all-features tests, hygiene, dependency policy, and packaging checks.
- RUSTUP_TOOLCHAIN=1.89.0 make docs-check: passed.
- ./packaging/check-repo-hygiene.sh: passed (repo-hygiene: ok).
- Independent read-only review: PASS; no concrete defects found. It also passed batch retrieval (29), MCP projection (36), renderer (34), web-fetch integration (128 passed, 1 live test ignored), parser conformance (6), static guards (68), and docs-check.
- Hosted CI: run 45127251813 completed successfully on the exact candidate SHA above; Linux CI, Windows portability, and macOS canonical/filesystem checks all passed.

## Residual limitations

- Common envelope overhead is intentionally excluded; the cap is on projected item-response payloads only.
- Brief mode must fetch and project the complete selected item window before enforcing its aggregate cap. Work remains bounded by existing item, deadline, and I/O limits, but it no longer short-circuits later Brief items solely from a pre-projection estimate.
- Payload counts are serialized Unicode characters, not bytes or model-token estimates.

No unresolved implementation finding blocks closure. Final recommendation: closed. No unresolved implementation finding or acceptance condition remains.

## Verification commands

RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
./packaging/check-repo-hygiene.sh
