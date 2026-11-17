# M003 — Opt-In Webpage Screenshot Artifacts

Status: proposed
Source: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M003
Primary class: capability
Baseline: `b7f7497` (`feat/auto-20261117-044c4885`)
References: accepted `plans/adrs/ADR-0015-opt-in-webpage-screenshot-artifacts.md`; accepted ADR-0001 and ADR-0014; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`; `plans/subsystems/adaptive-fetch-routing-roadmap.md`; `plans/implementation/adaptive-fetch-routing/002-batch-per-target-capability-parity.md`; `architecture/fetch.md`; `architecture/mcp.md`; `src/mcp/tools/common.rs`; `src/mcp/server.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/batch_fetch_item.rs`.
Dependencies: hard: M002 closed. Interface: ADR-0014 browser behavior. ADR-0015 is accepted and authorizes the narrow request/result contract and numeric caps. The completed feasibility preflight was static only; serializer/transport and browser-fidelity evidence remain implementation gates. This plan remains proposed and MUST NOT add production capture or public schema until M002 is closed.

## Objective

Add an explicitly requested, bounded screenshot artifact to `web_fetch` and each applicable `batch_fetch` web item, delivered through native MCP image content while preserving the existing structured fetch result. The screenshot is a webpage artifact from browser rendering, not part of PDF extraction. Calls without the opt-in field, or with it false, retain existing response behavior.

The accepted contract is specified in ADR-0015. Implement only after M002 closes, and confirm serializer/transport behavior and browser fidelity on the exact candidate.

## Non-goals

- No screenshots for PDFs, remote repository files, or local workspace files.
- No PDF rasterization, OCR, image extraction, or image analysis.
- No full-page screenshot, scrolling, clicking, keyboard input, challenge solving, or other browser interaction.
- No screenshot caching, filesystem artifact path, signed URL, Base64 image field in structured JSON, or diagnostic/log persistence.
- No change to ordinary `RenderPolicy::Auto`, browser defaults, lifecycle, SSRF/network policy, profile authorization, or the ten-tool surface.
- No fallback that claims success when browser rendering is unavailable.

## Current implementation evidence

- No screenshot capture implementation or screenshot result path exists in `src/`.
- `WebFetchArgs` already exposes `render` and `browser_profile`; browser availability and explicit errors are governed by ADR-0014.
- The common MCP result path maps `serde_json::Value` to `CallToolResult::structured`; batch web responses are currently JSON item payloads. Native image content must therefore be added through a deliberate result-assembly seam, not serialized as an accidental JSON string.
- ADR-0008 is accepted and text-only. Its PDF strategy remains untouched; this milestone implements only the narrow webpage screenshot exception accepted in ADR-0015.
- The 2026-11-17 static feasibility preflight found that pinned `rmcp` 3.9.0 can represent structured content and image blocks in one result type, and pinned `chromiumoxide` 0.9.1 exposes viewport-capable screenshot parameters and a `Vec<u8>` capture API. Accepted caps are 1280×720 CSS pixels at device scale factor 1 (921,600 pixels) and 4,194,304 encoded PNG bytes; batch images are paired with a preceding text association block containing original index and existing label. Serializer/transport behavior remains untested, and ADR-0015 is accepted.

## Invariants that cannot regress

- Screenshot request is opt-in and explicit. It forces browser capture even for an otherwise static HTTP page; contradictory explicit `render = "http_only"` is rejected as specified by ADR-0015.
- Browser-disabled, browser-unavailable, and feature-absent cases return the existing actionable capability errors; no silent HTTP fallback.
- Browser network interception, origin scoping, request and navigation deadlines, concurrency, lifecycle teardown, profile-origin validation, and no-interaction policy remain unchanged.
- Screenshot dimensions and encoded bytes are hard-bounded before result assembly. Image bytes do not enter JSON structured content, caches, logs, or diagnostics.
- Batch output associates every image block with exactly one successful item by original index and label; result ordering and telemetry remain unchanged.
- Screenshots are untrusted external visual content. Text sanitization does not imply image sanitization or verification.

## Implementation research (2026-11-17)

Read-only research on the M002 candidate confirmed the type-level preflight and found the concrete seams the milestone must change. No production code or schema changed.

### Result assembly

- `rmcp` 3.9.0 `CallToolResult` carries `content: Vec<ContentBlock>` and `structured_content: Option<Value>`; `ContentBlock::image(data, mime_type)` exists (`rmcp-3.9.0/src/model/content.rs:259-300`). `CallToolResult::structured(value)` produces exactly one JSON text block plus structured content (`rmcp-3.9.0/src/model.rs:4322-4330`). A screenshot result therefore appends an image block to that content vector and leaves JSON untouched.
- The only success conversion is `map_tool_result` (`src/mcp/tools/common.rs:461-475`), and it accepts only `Result<serde_json::Value, ToolError>`. The server helpers `respond` and `respond_response` (`src/mcp/server.rs:118-130`) apply advertisement filtering and `complete_response` before returning. Every tool runner returns `serde_json::Value` (`run_web_fetch_with_progress` at `src/mcp/tools/web_fetch.rs:180`, `run_batch_fetch_with_host_scope_and_progress` at `src/mcp/tools/batch_fetch.rs:155`).
- Consequence: image bytes need a new bounded attachment carrier (for example `ToolOutcome { value, images }`) threaded through those runners and both response helpers. `advertisement::filter_payload` must never inspect image bytes.
- `batch_fetch` can be offloaded through `spawn_task` (`src/mcp/server.rs:656-681`), whose closure currently reconstructs the result with `map_tool_result`; an offloaded screenshot batch would lose images unless the carrier is threaded into the task path or screenshot requests are made ineligible for offload. `web_fetch` itself is never task-offloaded.

### Capture point and bounds

- `run_browser_fetch` (`src/mcp/tools/common.rs:794-873`) holds the global browser semaphore and the per-origin browser permit, then calls `browser_fetch_with_policy`. `render_once` (`src/fetch/browser/navigate.rs:108-190`) creates a page with `width: 1280, height: 720`, navigates, extracts, and closes the page. Capture must run after extraction and before `page.close()` so it stays inside the lifecycle gate, both concurrency limits, the navigation deadline, and request interception.
- `chromiumoxide` 0.9.1 `Page::screenshot` (`src/page.rs:681`, implementation `src/handler/page.rs:396-458`) calls `Target.activateTarget` and then `Page.captureScreenshot`, returning the base64-decoded `Vec<u8>`. Viewport capture (`full_page = false`) issues no layout-metrics, device-metrics, or background-colour commands. Expected CDP additions are therefore activation plus capture.
- Page creation already requests the accepted 1280×720 viewport, and device scale factor is not overridden (default 1), so no extra viewport command is required. Enforce the pixel cap by reading the PNG IHDR width and height (big-endian at byte offsets 16 and 20) and rejecting anything above 1280×720; enforce the 4,194,304-byte encoded cap on the `Vec<u8>` length before base64 encoding. `base64` 0.22 is already a dependency (`Cargo.toml:87`); no image decoder or new dependency is needed.
- `Page::screenshot` activates the target first. If activation or capture fails, return an explicit capture error; do not return a text-only success.

### Request, validation, and cache

- Add optional `include_screenshot` (default false) to `WebFetchArgs` (`src/mcp/tools/web_fetch.rs:105-170`) and to `BatchFetchItem::Web`. Batch items are translated into `WebFetchArgs` in one place (`src/mcp/tools/batch_fetch_item.rs:45-81`), so the field is threaded there; `batch_fetch_plan_key` already serializes each item, so the distinct request shape gets a distinct plan key without extra work.
- Reject `include_screenshot = true` with explicit `render = "http_only"` in `run_web_fetch_inner` before any client work, matching the existing preflight style (`tests/batch_fetch_retrieval.rs::batch_prevalidates_target_options_before_fetching_any_item`). Other render values, including `auto`, are forced to browser capture.
- The browser branch builds its response from `BrowserFetchResult` and does not store it (`src/mcp/tools/web_fetch.rs:690-760`); raw and derived cache writes happen only on the HTTP path through `fetch_capturing` and `derive_from_raw`. A screenshot request therefore cannot read or pollute a cached text result. A regression test should still prove `cache_policy = "default"` performs a capture and that no image bytes appear in the structured JSON or raw cache entries.
- Batch results are collected by original index (`src/mcp/tools/batch_fetch.rs:545-727`), so one association text block plus one image block per successful screenshot item, in index order, uses the existing label and preserves order and telemetry.

### Test infrastructure gap

- There is no deterministic CDP fixture today. `tests/browser_transport.rs` covers classification, policy, configuration, and unavailable-browser errors; a successful navigation/render is exercised only by the ignored live smoke suite (`tests/browser_live_smoke.rs`). The planned "deterministic browser-transport fixture coverage for successful, oversized, unavailable, and timeout cases" therefore requires new infrastructure.
- Two options: a test-only renderer seam that returns a scripted `BrowserFetchResult` plus capture bytes, or a fake Chromium process speaking the required CDP subset. The seam is smaller, keeps caps and result assembly testable without a browser, and leaves live smoke as the fidelity gate; the fake process is closer to the real transport but substantially larger. This choice must be made before implementation because it determines where the byte and pixel caps are enforced.
- Serializer/transport proof can be an in-process MCP call that asserts the returned `CallToolResult`: the first content block is the JSON text, `structured_content` contains no image bytes, and the image block carries `image/png` with the expected base64 payload.
- Cap tests should cover 1280×720 (accepted), an over-width or over-height PNG (rejected), an encoded payload above 4 MiB (rejected), an empty capture (explicit error), browser feature disabled (existing capability error), and a capture timeout.

### Open decisions before implementation

1. Renderer seam versus fake Chromium process for deterministic tests.
2. Whether screenshot-bearing `batch_fetch` requests stay task-offload eligible; if so, the attachment carrier must reach the task result path.
3. Whether the contradictory-render error uses an existing validation code or needs a new stable code; the plan currently implies a validation error before I/O.
4. Placement of PNG header validation relative to the browser owner versus the MCP result assembly seam; the current evidence favours the browser owner so no oversized capture reaches assembly.

## Expected production changes

1. After M002 closes, add optional `include_screenshot` to `WebFetchArgs` and `BatchFetchItem::Web`, defaulting false and preserving omitted-field serialization behavior under accepted ADR-0015.
2. Add a browser capture operation that returns a viewport PNG within the accepted pixel, byte, and deadline caps. It performs no page interaction and only runs for web URLs.
3. Treat screenshot opt-in as an explicit browser request. Respect existing profile-origin checks and locks. If an explicit `render = "http_only"` conflicts, reject before starting transport; if browser capability is absent, use existing explicit browser errors.
4. Add a structured-result assembly path that preserves the existing JSON result and appends a native MCP image content block plus bounded association metadata. For batch, emit associations and images in input order; emit none for failed/non-screenshot items.
5. Do not store screenshot bytes in either cache tier. A screenshot request requires a new browser capture; existing text cache fields and cache status keep their documented meaning.
6. Update tool schema/contract documentation and docs/test inventory. No new MCP tool is registered.

## Ordered work packages

1. **Completed 2026-11-17 (static preflight only):** confirmed pinned `rmcp` support for structured JSON plus image content, identified the `chromiumoxide` viewport screenshot API, selected the accepted numeric caps (1280×720 CSS pixels at device scale factor 1; 4,194,304 encoded PNG bytes), and defined batch association as an adjacent text/image block pair using original index and existing label. Evidence is recorded in accepted ADR-0015. No production or public-schema changes were made; serializer/transport behavior remains an implementation gate.
2. After M002 is closed, add the optional request fields and schema/default compatibility tests for single and batch web fetches.
3. Implement bounded viewport capture in the existing browser owner and deterministic browser-transport fixture coverage for successful, oversized, unavailable, and timeout cases.
4. Extend the MCP result adapter to keep structured JSON and add the image block without placing bytes in JSON. Prove clients can associate the image with its request.
5. Add batch association/order tests for multiple successful screenshots mixed with failures and non-screenshot items; verify no cache, trace, or diagnostic persistence.
6. Update documentation and run focused plus broad gates on the exact candidate.

## Verification

- `cargo test --locked --features 'mock browser' --test browser_transport --test web_fetch_integration`
- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`
- `cargo test --locked --all-features --test mcp_tools --test mcp_tool_contract --test mcp_2026_protocol --test mcp_projection --test static_guards --test docs_tool_names`
- `cargo test --locked --no-default-features --features mock --test web_fetch_integration --test batch_fetch_retrieval`
- `make check`; `make docs-check`

Tests must use deterministic local fixtures/mocks; live browser smoke is optional operational evidence and cannot replace routine keyless tests. Record unavailable-browser cases and image-cap saturation cases explicitly.

## Documentation and static guards

- Update `architecture/fetch.md` with screenshot trigger, browser path, cache and bounds semantics.
- Update `architecture/mcp.md` and `docs/tool-matrix.md` with the opt-in field and MCP image-content association, including batch ordering.
- Update `docs/features.md` with the supported webpage-only scope and browser requirement; do not advertise PDF screenshots.
- Update `docs/test-inventory.md` for changed test counts and keep test placement consistent with `architecture/testing.md`.
- Keep all ten-tool inventory/schema guards and browser SSRF/lifecycle guards green; do not weaken them to accommodate image content.

## Acceptance and stop conditions

- When `include_screenshot` is absent or false, the existing tool arguments, structured JSON fields, result projection, and behavior are unchanged.
- When true, a bounded viewport screenshot is returned as native MCP image content only after an actual browser render; a browser capability failure is explicit.
- The screenshot is associated with the correct URL in single fetch and with the correct input index/label in batch; mixed batch results retain original order and existing telemetry.
- Numeric dimension and byte limits are exercised at, below, and above the cap; no screenshot bytes appear in structured JSON, caches, logs, or diagnostics.
- No screenshot is produced for PDF, repo, or local workspace targets, and no PDF image bytes are exposed.
- Stop if the pinned MCP implementation cannot carry structured JSON and image content together, if item association is ambiguous, if numeric bounds cannot be enforced, or if security requires weakening browser policy. Return for a revised ADR rather than switching to Base64 or local paths silently.

## Closure evidence

Write `plans/closure/adaptive-fetch-routing/003-status.md` with the candidate SHA, accepted ADR reference, exact image format/dimension/byte caps, browser and feature-absent matrix, single and batch item association/order, cache/log/diagnostic non-persistence evidence, tool inventory, focused/broad checks, docs, and every known limitation or unrun operational gate.
