# M002 — Batch Per-Target Capability Parity

Status: closed
Closure: `plans/closure/adaptive-fetch-routing/002-status.md`
Source: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M002
Primary class: capability
Baseline: `b7f7497` (`feat/auto-20261117-044c4885`)
References: `plans/000-long-term-specification.md` §§1–4; accepted ADR-0001, ADR-0008, ADR-0011, ADR-0014; `plans/subsystems/adaptive-fetch-routing-roadmap.md` §5; `plans/implementation/adaptive-fetch-routing/001-target-aware-fetch-routing.md`; `plans/implementation/pdf-ingestion-fidelity/008-local-workspace-and-batch-ingestion.md`; `src/core/batch_fetch.rs`; `src/mcp/tools/batch_fetch_item.rs`; `src/mcp/tools/repo_fetch.rs`; `src/mcp/tools/web_fetch.rs`; `tests/batch_fetch_retrieval.rs`.
Dependencies: interface dependency on M001's written target/handler contract and PDF M008's written local-PDF plus batch-`pdf` contract. Integration and local-PDF acceptance must run on the landed M001/PDF M008 implementations.

## Objective

Make each `batch_fetch` item express the target-specific routing and extraction controls needed to receive the same behavior as the corresponding single-target tool, without turning the batch schema into one untyped union of unrelated options. Web items use web-fetch routing controls; repo items use repo-fetch locator and source-selection controls; workspace items retain the repo workspace root/path authorization model. Preserve batch's input ordering, per-item failure behavior, deterministic IDs, budgets, and telemetry.

Parity in this milestone means parity for target resolution and extraction/source-selection behavior. It does not mechanically duplicate unrelated response projection or document-window arguments such as `response_detail`, `offset`, or `limit`. Screenshot output is owned by M003 and ADR-0015.

## Non-goals

- No new public MCP tool, tool rename, or removal of `web_fetch`/`repo_fetch`.
- No duplicate local PDF implementation or alternate local path semantics; PDF M008 owns local PDF extraction and the optional `pdf` field on batch web items.
- No page selection on `repo_fetch` or batch repo items unless a later accepted decision changes the current PDF M008 boundary.
- No screenshot capture or image content in this milestone.
- No changes to per-item identity, item ordering, total/item character budgets, or aggregate telemetry meanings.
- No attempt to give repo targets web-only controls such as browser profiles, nor to give web targets code-symbol selectors.

## Current implementation evidence

- At baseline `b7f7497`, `BatchFetchItem::Web` lacked the `web_fetch` PDF/render/profile route controls, and `BatchFetchItem::Repo` lacked source-selection fields such as symbol and match-text selection.
- The active worktree adds optional/defaulted `pdf`, `render`, and `browser_profile` fields to web items and repo source-selection fields (`symbol`, `symbol_kind`, `match_text`, block expansion, and `prefer_local`). Existing item fields remain serde-compatible.
- `src/mcp/tools/batch_fetch_item.rs` routes web items through the shared web-fetch handler and repo items through `run_repo_fetch_with_host_scope`; cache, profile, PDF, span, and response semantics remain owned by those handlers.
- `src/mcp/server.rs` carries `codegg/host-local-scope` into synchronous and offloaded batch execution. Repo items receive the same scope used by `repo_fetch`; the local-workspace resolver remains the authorization and safe-read owner.
- `src/core/fetch_locator.rs` maps workspace repo items to a root name and relative path. Batch preflight uses typed target resolution and validates per-target options before item I/O.
- Focused batch/repo/browser/no-default suites and MCP/static/documentation contract checks pass, including PDF M008 local extraction and batch page selection through the shared handler. Independent review caught and the follow-up fixed a target-specific validation mismatch: blank `repo` is valid for a workspace target when `path` is set, but remains invalid for remote targets; direct/batch workspace parity covers the blank value. `make check` (85 suites, 6,347 passed, 0 failed, 28 ignored), `make docs-check`, formatting, all-target clippy, and no-default compilation pass on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`. The earlier default-parallel Startpage failure was a test-isolation defect: each engine test module now removes only its own pass-cookie entry. Qualification also added the review-dated `RUSTSEC-2026-0543` waiver for unmaintained chrono (no safe upgrade; also reached through pinned `rmcp`/`schemars`) and a hermetic PATH for the fake-Plan9 installer fixture. Exact commands and outcomes are in `plans/closure/adaptive-fetch-routing/002-status.md`.

## Invariants that cannot regress

- Exactly ten MCP tools; the `Web` and `Repo` item tags and existing fields remain compatible, with additions optional and defaulted.
- Web-item PDF, browser, and profile semantics match the corresponding `web_fetch` controls and use the same cache-scope isolation and browser authorization rules.
- Repo-item host/ref/path, local `prefer_local` behavior, span selection, trust, and response shape match `repo_fetch` for equivalent requests.
- Workspace targets stay root-relative, enabled/authorized, and safe-opened. Local PDFs are handled only by PDF M008; PDFs remain excluded from indexing/repo maps.
- Invalid input is rejected before network I/O where the existing semantics permit; document-dependent failures remain per-item. Batch result order, labels, `stable_id`, `chars_returned`, truncation, aggregate budgets, and telemetry keep their meanings.
- No screenshot bytes enter cache keys or batch JSON; M003 owns screenshot content.

## Expected production changes

1. Extend the web item with the target-specific fields that already exist on `web_fetch`: `pdf`, `render`, and `browser_profile`, each optional/defaulted. Do not re-add the `pdf` field if PDF M008 has already landed it; reconcile its serialization and cache behavior rather than making a duplicate field. `include_screenshot` is reserved for M003.
2. Extend the repo item with the source-selection fields present on `repo_fetch` but absent from the batch item: symbol, symbol kind, match text, block expansion, maximum block lines, and `prefer_local`, using the same validators and semantics. Preserve existing structured locator and line-range fields.
3. Thread web options through validation, fetch execution, browser profile resolution/locking, PDF derivation, and every relevant derived-cache-key path. Profile cache scope must use the opaque profile ID. PDF settings must remain part of derived identity as specified by PDF M008.
4. Carry the trusted `codegg/host-local-scope` metadata from the server wrapper through batch execution and item futures, then call `run_repo_fetch_with_host_scope` for repo items. Direct and batch workspace calls must resolve the same authorized `LocalWorkspaceView`; internal/CLI callers without host metadata retain the existing unscoped configured-workspace behavior.
5. Route each item through the M001 target-resolution contract and owning per-target handler/service. Keep batch's bounded concurrency, preflight, per-item errors, result order, and item-index telemetry; do not call an MCP tool recursively.
6. Add direct-versus-batch equivalence tests for equivalent URL and repo requests. Compare target outcome, extracted document, warnings, trust, span selection, workspace authorization, and applicable cache behavior while excluding the documented batch envelope and batch stable ID.

## Ordered work packages

1. Reconcile the field list with the landed PDF M008 implementation and add serde defaults/round-trip coverage for absent and present options.
2. Add repo source-selection fields and validation using the same shared repo request validators; ensure `workspace` maps to the authorized root name plus relative path.
3. Propagate `codegg/host-local-scope` through the server/batch execution path and call `run_repo_fetch_with_host_scope`; preserve the existing no-host-context behavior for internal callers.
4. Thread web PDF/render/profile options through miss, cache-hit, conditional-revalidation, and raw-derive paths; verify no cross-mode or cross-profile cache hit.
5. Thread repo span/source-selection options into the scoped repo-fetch handler without changing local/remote response semantics.
6. Add preflight/runtime, mixed-target, output-order, partial-failure, profile-lock, cache-key, PDF-feature-absent, workspace-path, authorized-root, and host-scope rejection test cases.
7. Update schema and behavior docs and the suite inventory counts; append the changelog entry only when implementation lands.

## Verification

- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`
- `cargo test --locked --features 'mock pdf' --test local_workspace_integration` after PDF M008 lands
- `cargo test --locked --features 'mock pdf browser' --test web_fetch_integration --test browser_transport`
- `cargo test --locked --no-default-features --features mock --test batch_fetch_retrieval --test local_workspace_integration`
- `cargo test --locked --all-features --test mcp_tools --test mcp_tool_contract --test mcp_projection --test static_guards --test docs_tool_names`
- `make check`; `make docs-check`

All integration cases must use mocks/fixtures and remain network-free. Record exact commands, outcomes, skips, and failures in the closure record.

## Documentation and static guards

- Update `docs/tool-matrix.md` and `architecture/mcp.md` for per-item fields and target-specific parity.
- Update `architecture/fetch.md` for route/cache behavior and `architecture/local-workspace.md` only if workspace resolution changes.
- Update `docs/features.md` and `docs/config.md` only for observed behavior/config changes. No config addition is planned here.
- Update `docs/test-inventory.md` for changed behavioral suite counts; do not add a suite without the corresponding `architecture/testing.md` placement update.
- Keep tool-name, stable registration count, schema, cache-profile isolation, local path safety, and ordinary module-size guards green. Do not relax a guard to fit this milestone.

## Acceptance and stop conditions

- Every supported web route control listed above works on an individual batch web item and has the same target outcome as an equivalent `web_fetch` call.
- Repo span/source-selection fields work per item and match `repo_fetch` for equivalent remote and workspace locators.
- When a host-local scope is supplied, batch workspace reads are restricted to that same authorized view as `repo_fetch`; a root outside the scope is rejected. Calls without host metadata retain the existing configured-workspace behavior.
- A local PDF batch item succeeds only through PDF M008's workspace-root semantics; an absolute path, traversal path, unapproved root, or `file://` URL is rejected.
- Mixed batches preserve original order and stable IDs; one runtime item failure does not discard unrelated successful results; aggregate budgets and telemetry remain stable.
- Cache hits cannot cross PDF options, browser transport policy, or opaque profile scope.
- Stop if parity requires changing a target's safety/trust semantics, adding page selection to repo tools, weakening PDF M008's local-path boundary, or changing the ten-tool surface. Return for a separate ADR/plan rather than extending scope silently.

## Closure evidence

Write `plans/closure/adaptive-fetch-routing/002-status.md` with the candidate SHA, per-target option matrix, direct-vs-batch equivalence results, cache/profile isolation, host-local-scope pass/reject evidence, local-PDF integration (or explicit blocked dependency), mixed-batch order/telemetry evidence, schema inventory, focused/broad checks, documentation, and unresolved or unrun items.
