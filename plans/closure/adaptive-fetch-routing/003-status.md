# M003 — Opt-In Webpage Screenshot Artifacts — Closure

Status: closed
Recommendation: closed; the accepted M003 criteria pass on the exact implementation candidate.
Implementation candidate: `59ff64ad00940573ee520ecf19d7b5c0abcb1c87`
Baseline: `b7f74971c09017ad139084db7ddbfa0508d1871a`
Plan: `plans/implementation/adaptive-fetch-routing/003-opt-in-webpage-screenshot-artifacts.md`
Roadmap: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M003
Accepted decision: `plans/adrs/ADR-0015-opt-in-webpage-screenshot-artifacts.md`

## Candidate disposition

The implementation candidate was tested after commit `59ff64ad00940573ee520ecf19d7b5c0abcb1c87`. `make check` and `make docs-check` both passed on that exact SHA. The focused Python installed-wheel schema-parity test also passed on this candidate (1 passed). This closure record and the corresponding plan, roadmap, and registry status updates are committed immediately after the implementation candidate. The candidate was local and unpushed when these local gates ran; hosted PR checks are reported separately. No source changes were made after the candidate commit.

## Acceptance matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Explicit opt-in and compatibility | `include_screenshot` is optional on `web_fetch` and web batch items. `src/mcp/tools/tests.rs::screenshot_request_fields_are_omitted_when_absent_or_false` proves absent and false serialize identically; full and compact schema checks live in `tests/mcp_schema_slimming.rs`. | pass |
| Viewport PNG bounds | `src/fetch/browser/capture.rs` validates PNG signature/IHDR and rejects zero or over-limit dimensions and bytes. Its tests accept 1280×720 and 4 MiB boundaries and reject malformed headers, 1281×720, 1280×721, and 4 MiB + 1. | pass |
| Native MCP image result without JSON image bytes | `src/mcp/tools/tests.rs::screenshot_web_fetch_uses_browser_and_keeps_image_out_of_json_and_cache` asserts the JSON text plus `image/png` block, exact PNG payload, and absence of encoded bytes from structured content. | pass |
| Cache non-persistence | The same deterministic capture test requests the default cache policy and verifies raw and derived cache entry counts remain zero. Image bytes remain in the out-of-band `ToolOutcome` until MCP response assembly; no cache or diagnostic writer is added. | pass; no separate live log-capture test |
| Render conflict and explicit capability failures | `screenshot_conflict_fails_before_browser_io` proves explicit `http_only` rejects before browser I/O. `screenshot_capability_errors_remain_explicit` covers disabled and unavailable browsers; the no-browser-feature test covers `browser_not_compiled_in`. | pass |
| PDF and other non-webpage targets | `src/fetch/browser/navigate.rs` skips capture and appends a warning for a detected PDF; `pdf_document_detection_skips_screenshot_capture` covers PDF detection. The screenshot field exists only on web fetch arguments and web batch items, not repo or workspace targets. | pass |
| Batch association, ordering, failures, and offload | `screenshot_batch_associations_follow_successful_input_order` verifies image pairs for original indices 0 and 3, while a plain item and failed item emit no image. `src/mcp/server.rs` excludes screenshot-bearing batches from task offload. | pass |
| Tool count, schema pins, and docs | The MCP inventory remains ten tools. Full fingerprint `dd42e0167785cf24` (78,072 bytes); compact fingerprint `c82cd0797c239e12` (22,540 bytes). Documentation and test inventory are updated. | pass |

## Verification on `59ff64ad00940573ee520ecf19d7b5c0abcb1c87`

| Command | Outcome |
|---|---|
| `make check` | pass: formatting, clippy, no-default-features check, all-features tests, hygiene, dependency policy, and packaging checks; all-features tests: 6,359 passed, 28 ignored |
| `make docs-check` | pass |
| Python installed-wheel schema parity (`test_typed_requests_cover_canonical_rust_input_schemas`) | pass: 1 passed on the exact candidate |
| `cargo test --locked --all-features --lib screenshot_` | pass |
| `cargo test --locked --no-default-features --features mock --lib screenshot_capability_error_when_browser_feature_is_absent` | pass |
| Full and compact schema, tool-contract, static-guard, and docs-tool tests | pass; pinned advertisements match the values above |
| `cargo test --locked --features mock` | pass: 6,332 passed, 5 ignored |

## Documentation and planning

- `architecture/fetch.md`, `architecture/mcp.md`, and `architecture/testing.md`
- `docs/features.md`, `docs/tool-matrix.md`, `docs/test-inventory.md`, and `CHANGELOG.md`
- M003 implementation plan, adaptive-fetch roadmap, and `plans/registry.md` synchronized in the closure commit

## Limitations and unrun gates

- Live browser smoke was not run. Deterministic browser overrides prove capture/result wiring and bounds, but do not establish real Chromium visual fidelity or live transport behavior.
- The initial hosted PR run `44324947570` was on the older head `c17403c4cad959df2edc6b1f2a60be2da0f96792`; its Python sdist, ARMv7, and installed-wheel smoke failures all reported the missing `include_screenshot` property on `WebFetchRequest`. The type fix is included in candidate `59ff64ad00940573ee520ecf19d7b5c0abcb1c87`, and hosted checks must be rerun after push; this stale-head run is not acceptance evidence.
- No dedicated runtime log/diagnostic capture test was added. Static review confirms PNG bytes are carried only in the ephemeral out-of-band result path and are not passed to cache, logging, or diagnostic persistence code.
