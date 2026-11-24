# A002 — Batch Per-Target Capability Parity — Closure

Status: closed
Recommendation: closed; every M002 acceptance criterion passes on the exact candidate. The candidate is local and unpushed; no hosted CI result is claimed.
Implementation candidate: `24f045729f8409dadc96b4c2455253d8a9c7b6ec`
Baseline: `b7f74971c09017ad139084db7ddbfa0508d1871a`
Plan: `plans/archive/implementation/adaptive-fetch-routing/002-batch-per-target-capability-parity.md`
Roadmap: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M002
Related: ADR-0001, ADR-0008, ADR-0011, ADR-0014; `plans/implementation/pdf-ingestion-fidelity/008-local-workspace-and-batch-ingestion.md`

## Candidate disposition

Qualification is SHA-specific. The candidate is the implementation commit; this closure record and the registry and roadmap updates are committed immediately after it. The gates ran on the candidate tree before the commit was created, and no source changed between the gated tree and `24f045729f8409dadc96b4c2455253d8a9c7b6ec`.

The candidate also contains the M001 target-resolution seam and the PDF M008 local-PDF and batch contract that M002 depends on. M001's own closure record remains outstanding and is not claimed here.

## Acceptance matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Web item accepts `pdf`, `render`, and `browser_profile`, optional and defaulted, with `web_fetch` semantics | `src/mcp/tools/batch_fetch_item.rs:45-81` translates the item into `WebFetchArgs`; `src/core/batch_fetch.rs::batch_fetch_target_options_default_and_round_trip` proves omitted, present, and round-trip serialization; `tests/batch_fetch_retrieval.rs::batch_web_target_options_match_web_fetch` compares URL, status, text, document, focus, trust, warnings, cache status, and representation against a direct call | pass |
| PDF page options work per web item and cannot cross derived cache entries | `batch_web_target_options_match_web_fetch`; `batch_pdf_page_options_do_not_cross_derived_cache_entries`; `batch_pdf_invalid_page_syntax_fails_before_any_fetch`; `batch_pdf_out_of_range_is_item_local_and_preserves_mixed_order`; `batch_pdf_options_are_ignored_for_html_and_metadata_only_matches_web_fetch`; `batch_workspace_pdf_matches_repo_fetch` | pass |
| Repo item exposes symbol, symbol kind, match text, block expansion, maximum block lines, and `prefer_local`, matching `repo_fetch` | `tests/batch_fetch_retrieval.rs::batch_repo_span_options_match_workspace_repo_fetch`; shared validators and `run_repo_fetch_with_host_scope` in `src/mcp/tools/batch_fetch_item.rs:237` | pass |
| Host-local scope restricts batch workspace reads to the authorized view and never widens configured roots | `src/mcp/tools/tests.rs::batch_workspace_fetch_honors_host_local_scope_without_widening`: authorized root returns its text, an out-of-scope configured root is rejected with `unknown workspace root`, and an unscoped internal call keeps the configured roots | pass |
| Local PDFs succeed only through PDF M008 workspace-root semantics; absolute, traversal, unapproved-root, and `file://` inputs are rejected | `batch_workspace_pdf_matches_repo_fetch`; `tests/local_workspace_integration.rs::local_repo_fetch_extracts_extension_and_magic_pdfs_with_local_trust`, `local_repo_fetch_pdf_respects_the_configured_file_budget`, `local_repo_fetch_reports_pdf_disabled_without_falling_back_to_text`, `local_repo_fetch_reports_pdf_feature_unavailable`, `local_repo_fetch_maps_pdf_without_extractable_text_to_empty_success`, `local_workspace_fetch_still_rejects_non_pdf_binary_files`; `tests/batch_fetch_retrieval.rs::batch_locator_safety_preserved` | pass |
| Mixed batches preserve order and stable IDs; one item failure does not discard unrelated results; budgets and telemetry stay stable | `batch_web_and_workspace_repo_in_one_batch`; `batch_failure_isolation`; `batch_pdf_out_of_range_is_item_local_and_preserves_mixed_order`; `batch_aggregate_truncation_is_explicit`; `batch_focus_with_cache_hit` | pass |
| Cache hits cannot cross PDF options, browser transport policy, or opaque profile scope | `batch_pdf_page_options_do_not_cross_derived_cache_entries` covers PDF derivation; profile scope uses `CacheScope::Profile(ProfileId::opaque(...))` at `src/mcp/tools/web_fetch.rs:430`, shared by batch through `WebFetchArgs`, with scope inequality covered by `src/fetch/cache/tests.rs`; the browser branch builds its response without a cache write. No dedicated batch profile integration case exists (see Limitations) | pass with limitation |
| Invalid target options are rejected before item I/O | `batch_prevalidates_target_options_before_fetching_any_item`; `batch_pdf_invalid_page_syntax_fails_before_any_fetch`; `batch_locator_safety_preserved`; `src/mcp/tools/tests.rs` locator helpers | pass |
| Ten-tool surface, schema, stable IDs, and guards stay intact | `tests/mcp_tool_contract.rs` (ten tools, pinned full and compact advertisements); `tests/mcp_schema_slimming.rs::batch_fetch_advertises_per_target_controls_in_full_and_compact_schemas`; `tests/docs_tool_names.rs`; `tests/static_guards.rs` | pass |

## Command results — `24f045729f8409dadc96b4c2455253d8a9c7b6ec`

| Command | Outcome |
|---|---|
| `cargo fmt --check` | pass |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | pass |
| `cargo check --locked --no-default-features` | pass |
| `cargo test --locked --all-features` | pass: 85 suites, 6,347 passed, 0 failed, 28 ignored |
| `make hygiene` | pass |
| `make dependency-policy` | pass with the documented `RUSTSEC-2026-0543` waiver |
| `make packaging-check` | pass |
| `make check` | pass |
| `make docs-check` | pass |
| `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval` | pass: 22 passed |
| `cargo test --locked --features 'mock pdf' --test local_workspace_integration` | pass: 15 passed |
| `cargo test --locked --features 'mock pdf browser' --test web_fetch_integration --test browser_transport` | pass: 99 passed and 1 ignored; 75 passed |
| `cargo test --locked --no-default-features --features mock --test batch_fetch_retrieval --test local_workspace_integration` | pass: 17 passed; 12 passed |
| `cargo test --locked --all-features --test mcp_tools --test mcp_tool_contract --test mcp_projection --test static_guards --test docs_tool_names` | pass |

## Qualification corrections

1. **Pass-cookie test isolation.** `startpage_tests.rs` and the `mojeek.rs` test module each cleared the process-wide `pass_cookie_cache()` under separate locks, so a default-parallel run could erase another engine's entry. Each test now removes only its own provider entry. No production behavior changed.
2. **Traversal error parity.** M002's locator preflight returned `path must not contain '..'` while the repo-fetch handler returned `path must not contain '..' (path traversal)`. `src/core/fetch_locator.rs` now uses the established wording, preserving the user-visible error string that `tests/research_workflow.rs::workspace_fetch_rejects_path_traversal` asserts.
3. **Dependency policy.** `RUSTSEC-2026-0543` reports unmaintained chrono with no safe upgrade; chrono is a direct dependency and is also reached through pinned `rmcp` 3.9.0 and `schemars` 1.2.2, so removing the direct dependency would not clear the notice. An exact, review-dated waiver was added to `deny.toml` and `docs/dependency-security.md` (review by 2026-12-31), following the four existing scoped waivers. No wildcard, vulnerability, or source-policy waiver was added.
4. **Packaging fixture PATH.** The fake-Plan9 case ran `install.sh` with the ambient PATH, so the workstation `rm` shim inherited the fake `uname` and failed cleanup with `mavis-trash: unsupported OS: Plan9`. That case now uses a hermetic system PATH; `install.sh` still sees the fake platform and exercises the Cargo fallback.
5. **Schema documentation.** The M002 worktree had removed every `BatchFetchItem` field description and added `#[allow(missing_docs)]`, which broke `tests/schema_identity_registry.rs::batch_fetch_repo_host_description_lists_all_aliases` and silently narrowed the pre-existing schema contract. Field descriptions are restored, new fields are documented, and the pinned advertisement surfaces were re-pinned deliberately: full fingerprint `5a5ccb9fa49f9e95` at 77,800 bytes; compact fingerprint `748e5c2d18df1678` at 22,338 bytes. The advertised-surface budget moved from 75,200 to 78,250 bytes with the measured +2,532 increment recorded in the assertion message.

## Documentation

- `architecture/mcp.md`, `architecture/fetch.md`, `architecture/local-workspace.md`, `architecture/overview.md`, `architecture/testing.md`
- `docs/tool-matrix.md`, `docs/features.md`, `docs/config.md`, `docs/test-inventory.md`, `docs/dependency-security.md`
- `CHANGELOG.md`
- Plan and roadmap status, `plans/registry.md`

## Limitations and unrun gates

- The candidate is local. No push, PR, hosted CI, or OS matrix ran on this SHA; hosted `make ci` remains unrun.
- M001's closure record remains outstanding on the same implementation.
- Browser route and profile parity is exercised structurally and through the shared handler and validation paths; no live browser render is part of the keyless gate.
- Profile cache isolation relies on the shared `CacheScope::Profile` code and cache unit tests rather than a dedicated batch integration test.
- The `RUSTSEC-2026-0543` waiver must be reviewed by 2026-12-31 or replaced when `rmcp`/`schemars` no longer require chrono.
- The pinned advertisement values and the advertised-surface budget are the new baseline recorded above.
