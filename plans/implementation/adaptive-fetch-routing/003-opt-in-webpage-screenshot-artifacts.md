# M003 — Opt-In Webpage Screenshot Artifacts

Status: closed
Closure: `plans/closure/adaptive-fetch-routing/003-status.md`
Source: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M003
Primary class: capability
Baseline: `b7f7497` (`feat/auto-20261117-044c4885`)
References: accepted `plans/adrs/ADR-0015-opt-in-webpage-screenshot-artifacts.md`; accepted ADR-0001 and ADR-0014; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`; `plans/subsystems/adaptive-fetch-routing-roadmap.md`; `plans/implementation/adaptive-fetch-routing/002-batch-per-target-capability-parity.md`; `plans/closure/adaptive-fetch-routing/002-status.md`; `architecture/fetch.md`; `architecture/mcp.md`; `src/mcp/tools/common.rs`; `src/mcp/server.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/batch_fetch_item.rs`.
Dependencies: M002 is closed (`plans/closure/adaptive-fetch-routing/002-status.md`, candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`). Interface: ADR-0014 browser behavior. ADR-0015 is accepted and authorizes the narrow request/result contract and numeric caps. The feasibility preflight was static only; deterministic in-process image serialization is covered by the implementation tests, and live browser fidelity remains unrun as recorded in the closure.

## Objective

Add an explicitly requested, bounded screenshot artifact to `web_fetch` and each applicable `batch_fetch` web item, delivered through native MCP image content while preserving the existing structured fetch result. The screenshot is a webpage artifact from browser rendering, not part of PDF extraction. Calls without the opt-in field, or with it false, retain existing response behavior.

## Non-goals

- No screenshots for PDFs, remote repository files, or local workspace files.
- No PDF rasterization, OCR, image extraction, or image analysis.
- No full-page screenshot, scrolling, clicking, keyboard input, challenge solving, or other browser interaction.
- No screenshot caching, filesystem artifact path, signed URL, Base64 image field in structured JSON, or diagnostic/log persistence.
- No change to ordinary `RenderPolicy::Auto`, browser defaults, lifecycle, SSRF/network policy, profile authorization, or the ten-tool surface.
- No fallback that claims success when browser rendering is unavailable.
- No new MCP tool, output-schema field, or batch telemetry field.

## Current implementation evidence

- At the planning baseline `b7f7497`, no screenshot capture implementation or screenshot result path existed in `src/`.
- `WebFetchArgs` exposes `render` and `browser_profile`; capability checks are centralized in `validate_batch_route_options` (`src/mcp/tools/web_fetch.rs:23`), which both `web_fetch` and batch items call.
- `run_browser_fetch` (`src/mcp/tools/common.rs:794`) holds the global browser semaphore and per-origin browser permit, then calls `browser_fetch_with_policy`. `render_once` (`src/fetch/browser/navigate.rs:108`) creates a 1280×720 page, navigates, extracts, and closes it.
- The common success conversion is `map_tool_result` (`src/mcp/tools/common.rs:461`); it accepts only `serde_json::Value`. `respond` and `respond_response` (`src/mcp/server.rs:118-130`) are the only server result seams.
- `batch_fetch` is task-offload eligible (`src/mcp/server.rs:656-681`); `web_fetch` is not.
- ADR-0008 remains text-only for PDFs. This milestone implements only the ADR-0015 webpage exception.

## Invariants that cannot regress

- Screenshot request is opt-in and explicit. It forces browser capture even for a static HTTP page; explicit `render = "http_only"` conflicts.
- Browser-disabled, browser-unavailable, and feature-absent cases return the existing actionable capability errors; no silent HTTP fallback.
- Browser network interception, origin scoping, deadlines, concurrency, lifecycle teardown, profile-origin validation, and no-interaction policy remain unchanged.
- Screenshot dimensions and encoded bytes are hard-bounded before result assembly. Image bytes never enter JSON, caches, logs, or diagnostics.
- Batch output associates every image with exactly one successful item by original index and label; result ordering and telemetry remain unchanged.
- Screenshots are untrusted external visual content; text sanitization does not imply image verification.
- The ten-tool surface, tool names, stable IDs, batch order, budgets, and cache semantics are unchanged.

## Implementation research (2026-11-17)

Read-only research on the M002 candidate confirmed the type-level preflight and located the seams. The details remain authoritative for this plan.

### Result assembly

- `rmcp` 3.9.0 `CallToolResult` carries `content: Vec<ContentBlock>` and `structured_content: Option<Value>`; `ContentBlock::image(data, mime_type)` exists (`rmcp-3.9.0/src/model/content.rs:259-300`). `CallToolResult::structured(value)` emits one JSON text block plus structured content (`rmcp-3.9.0/src/model.rs:4322-4330`).
- Only `map_tool_result` converts success values, and every runner returns `serde_json::Value`. Image bytes therefore need a bounded out-of-band carrier threaded to the server seam.
- `batch_fetch` task offload reconstructs the result with `map_tool_result`; an offloaded screenshot batch would lose images.

### Capture point and bounds

- `Page::screenshot` (`chromiumoxide-0.9.1/src/page.rs:681`, implementation `src/handler/page.rs:396-458`) activates the target, executes `Page.captureScreenshot`, and base64-decodes to `Vec<u8>`. Viewport mode (`full_page = false`) issues no layout-metrics or device-metrics commands.
- The page is already created at 1280×720 with default device scale factor 1, matching the accepted caps. `Page::screenshot` must run after extraction and before `page.close()`, inside `render_once`.
- Accepted caps: 1280×720 CSS pixels (921,600 pixels) and 4,194,304 encoded PNG bytes. Validate the PNG signature and IHDR width/height at byte offsets 16 and 20 before base64 encoding; reject empty, truncated, non-PNG, over-dimension, and over-byte captures.
- `base64` 0.22 is already a dependency (`Cargo.toml:87`); no decoder or new dependency is needed.

### Request, validation, cache, and tests

- Both `web_fetch` and batch web items route through `validate_batch_route_options`, so one conflict rule covers both.
- Batch items are translated to `WebFetchArgs` in `src/mcp/tools/batch_fetch_item.rs:45-81`; the existing plan key serializes items, so the new field automatically separates plan keys.
- The browser branch builds its response without a cache write (`src/mcp/tools/web_fetch.rs:690-760`); cache writes happen only on the HTTP path.
- There is no deterministic CDP fixture. `tests/browser_transport.rs` covers classification, policy, configuration, and unavailable-browser errors; successful rendering is live-smoke only (`tests/browser_live_smoke.rs`, `#[ignore]`).

## Design decisions

1. **Deterministic test seam: scripted browser executor, not a fake Chromium process.** A `#[cfg(all(feature = "mock", feature = "browser"))]` override on `ServerState` is consulted by `run_browser_fetch` after permits and lifecycle retrieval, before `browser_fetch_with_policy`. It returns a scripted `BrowserFetchOutcome` (or error). This tests capture wiring, bounds rejection propagation, result assembly, batch association, offload eligibility, and cache non-persistence without a browser. `ScreenshotCapture` validation is tested directly as a pure function. The production path remains thin; live smoke stays the browser-fidelity gate. A fake CDP process is rejected as disproportionate for this milestone.
2. **Screenshot-bearing `batch_fetch` requests are not task-offloaded.** The request stays synchronous (`CallToolResponse::Complete`). Threading image attachments through the task result path would expand the task contract for no user benefit. `web_fetch` is already never offloaded.
3. **Contradictory render is a pre-I/O conflicting-arguments error.** `include_screenshot = true` with an explicitly supplied `render = "http_only"` returns `ToolErrorCode::ConflictingArguments` on `render` before any client or browser work. An omitted `render` whose configured default is `http_only` is not a conflict; the explicit screenshot request overrides the default per ADR-0015.
4. **Bounds are enforced in the browser owner.** `capture_viewport` constructs a validated `ScreenshotCapture`; no raw capture bytes cross the module boundary. The MCP layer only base64-encodes validated bytes.
5. **No structured-JSON screenshot field.** `WebFetchResponse`, `BatchFetchResponse`, and their output schemas are unchanged. Single-fetch association is the image block adjacent to the result; batch association is a preceding text block naming the original index and label. This is the strictest reading of ADR-0015's compatibility promise and avoids schema churn.
6. **Non-webpage targets are never captured.** If the captured main response content type is `application/pdf` (or the final path ends `.pdf` with a PDF content type), capture is skipped and a warning is appended: `screenshot not produced: target resolved to a PDF document`. The normal browser text/document result is returned without an image and without a second fetch.
7. **PNG only.** Request `CaptureScreenshotFormat::Png`, `full_page = false`, `capture_beyond_viewport = false`. No JPEG path, no format negotiation.
8. **Association labels are sanitized and bounded.** Batch association text uses the existing control-character stripping and sanitization helpers, caps the label at 200 characters, and contains the original zero-based item index.

## Expected production changes

1. Add `include_screenshot: Option<bool>` to `WebFetchArgs` and to `BatchFetchItem::Web`, serde-defaulted and omitted from serialization when absent.
2. Extend `validate_batch_route_options` with the screenshot flag: reject the explicit `http_only` conflict, otherwise treat the request as an explicit browser request for capability checks.
3. Add `src/fetch/browser/capture.rs` with the accepted caps, `ScreenshotCapture`, `CaptureError`, pure PNG-header validation, and the viewport capture call.
4. Replace the browser fetch return path with `BrowserFetchOutcome { result, screenshot }`; keep thin `browser_fetch`/`browser_fetch_with_policy` wrappers for existing callers. Capture after extraction and before page close, under the existing navigation timeout.
5. Add an out-of-band MCP attachment carrier and conversion (`ToolOutcome`, `ImageAttachment`, `map_tool_outcome`, server `respond_outcome`). Keep `map_tool_result` for all other tools.
6. Thread the carrier through `run_web_fetch_with_progress`, `run_batch_fetch_with_host_scope_and_progress`, and the batch item executor. The JSON-only `run_web_fetch`/`run_batch_fetch` wrappers map to `.value`; the two internal test call sites on the progress runners assert on `.value`.
7. Make screenshot-bearing `batch_fetch` ineligible for task offload in the server wrapper.
8. Update schema/contract pins, architecture and tool documentation, the test inventory, and the changelog.

## Production design

### `src/fetch/browser/capture.rs`

```rust
pub const MAX_SCREENSHOT_WIDTH: u32 = 1280;
pub const MAX_SCREENSHOT_HEIGHT: u32 = 720;
pub const MAX_SCREENSHOT_BYTES: usize = 4 * 1024 * 1024;

pub enum CaptureError {
    Empty,
    NotPng,
    TruncatedHeader,
    ZeroDimension,
    Dimensions { width: u32, height: u32 },
    TooLarge { bytes: usize, cap: usize },
}

pub struct ScreenshotCapture { /* validated png, width, height */ }

impl ScreenshotCapture {
    pub fn try_new(png: Vec<u8>) -> Result<Self, CaptureError>;
    pub fn png(&self) -> &[u8];
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
}

pub(crate) async fn capture_viewport(
    page: &chromiumoxide::Page,
) -> Result<ScreenshotCapture, BrowserFetchError>;
```

`try_new` checks length, the 8-byte PNG signature, an `IHDR` tag at bytes 12–16, nonzero big-endian width and height at bytes 16–24, the dimension cap, and the byte cap. It does not decode image data or verify CRCs.

### `src/fetch/browser/navigate.rs`

```rust
pub struct BrowserFetchOutcome {
    pub result: BrowserFetchResult,
    pub screenshot: Option<ScreenshotCapture>,
}

pub async fn browser_fetch_outcome(
    lifecycle: &Arc<BrowserLifecycle>,
    url: &str,
    config: &BrowserConfig,
    sanitize_output: bool,
    policy: &RenderPolicy,
    capture: bool,
) -> Result<BrowserFetchOutcome, BrowserFetchError>;
```

`render_once` gains the capture flag, checks the captured main-response content type before capture, wraps `capture_viewport` in the remaining navigation timeout, and returns `CaptureFailed`/`Timeout` explicitly. Existing wrappers return `outcome.result` and pass `capture = false`.

### `src/mcp/state.rs`

```rust
#[cfg(all(feature = "mock", feature = "browser"))]
pub struct MockBrowserRequest { pub url: String, pub capture: bool }

#[cfg(all(feature = "mock", feature = "browser"))]
pub type BrowserFetchOverride = Arc<
    dyn Fn(MockBrowserRequest) -> Result<BrowserFetchOutcome, BrowserFetchError>
        + Send + Sync,
>;
```

Add `#[cfg(all(feature = "mock", feature = "browser"))] pub browser_fetch_override: Option<BrowserFetchOverride>` to `ServerState`, initialize it to `None` in both constructors, and skip it in `Debug`. The combined gate keeps the no-default `mock` build compiling without the browser module. `run_browser_fetch` consults it after acquiring permits and resolving the lifecycle, before `browser_fetch_outcome`.

### `src/mcp/tools/common.rs` and `src/mcp/server.rs`

```rust
pub(crate) struct ImageAttachment {
    pub association: Option<String>,
    pub png: Vec<u8>,
}

pub(crate) struct ToolOutcome {
    pub value: serde_json::Value,
    pub images: Vec<ImageAttachment>,
}

pub(crate) fn map_tool_outcome(
    result: Result<ToolOutcome, ToolError>,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData>;
```

On success, `map_tool_outcome` builds `CallToolResult::structured(value)`, then appends, per image, the optional association text block and `ContentBlock::image(STANDARD.encode(png), "image/png")`. On error it delegates to `map_tool_result`. `respond_outcome` is the server wrapper; `respond` remains for JSON-only tools.

### Tool flow

- `run_web_fetch` keeps its JSON-only signature; `run_web_fetch_with_progress` and the prepared-client variant return `ToolOutcome`. The server calls `respond_outcome` for `web_fetch`. The one existing with-progress call in `tests/mcp_tools.rs:924` asserts on `.value`.
- `batch_fetch_item` returns the item JSON plus an optional `ImageAttachment`; `run_batch_fetch_with_host_scope_and_progress` collects attachments by original index into `Vec<Option<ImageAttachment>>` and emits them in input order. `run_batch_fetch` keeps its JSON-only signature; the two host-scope unit-test calls in `src/mcp/tools/tests.rs` assert on `.value`.
- `batch_fetch` in `server.rs` computes `eligible = is_eligible(...) && !contains_screenshot(&args.items)`.

## Ordered work packages

1. **Capture primitives and bounds.** Add `capture.rs`, constants, `ScreenshotCapture`, `CaptureError`, `capture_viewport`, and pure validation tests. Acceptance: boundary unit tests pass without a browser; no production tool path changes yet.
2. **Browser outcome plumbing and mock seam.** Add `BrowserFetchOutcome`, the capture flag in `render_once`, non-webpage skip and warning, timeout handling, wrappers, and the `ServerState` mock override. Re-export `BrowserFetchOutcome` and `ScreenshotCapture` through `browser/mod.rs` and the crate fetch surface. Acceptance: existing browser suites pass unchanged with capture disabled; mock success/error propagation is covered.
3. **Request fields and validation.** Add `include_screenshot` to both request types, extend `validate_batch_route_options`, force the browser path, and preserve feature-off errors. Acceptance: serde/default and conflict tests pass; no I/O on conflict.
4. **Result assembly.** Add `ToolOutcome`, `ImageAttachment`, `map_tool_outcome`, and `respond_outcome`; emit the single-fetch image block. Acceptance: in-process `CallToolResult` has the JSON text block, the image block, and a JSON-free `structured_content`.
5. **Batch association and offload.** Thread the carrier through batch items and the runner, sanitize association text, preserve order, and disable offload for screenshot batches. Acceptance: mixed-batch order/failure tests; screenshot batches return `Complete`.
6. **Cache and non-persistence proof.** Exercise `cache_policy = "default"` capture, assert no cache entry is created for the screenshot request, and assert no base64/PNG bytes appear in JSON, warnings, progress, or diagnostics. Acceptance: dedicated integration tests.
7. **Schema, docs, and gates.** Update docs, inventory, and changelog; measure and re-pin advertisement surfaces; run the full gate matrix.

## Test plan

| Area | Case | Placement |
|---|---|---|
| Bounds | 1280×720 accepted; 1281×720 and 1280×721 rejected; zero rejected; bad signature/truncated IHDR rejected; exactly 4 MiB accepted; 4 MiB + 1 rejected | `capture.rs` unit tests |
| Request | omitted/false serialize identically; true round-trips; `render=http_only` + true rejected before I/O; `render=auto` + true forces browser | `tests/browser_transport.rs`, `tests/batch_fetch_retrieval.rs` |
| Capability | feature off → existing `browser_not_compiled_in`; disabled → `browser_disabled`; no executable → `browser_unavailable` | `tests/browser_transport.rs`, no-default run |
| Single result | JSON text block first, image block second, `image/png`, valid base64, no bytes in `structured_content` | `tests/mcp_tools.rs`, `tests/mcp_2026_protocol.rs` |
| Capture failure | capture error and timeout return explicit errors; no text-only success | `tests/browser_transport.rs` mock |
| Non-webpage | PDF content type skips capture, returns text/document + warning, no image | `tests/browser_transport.rs` mock |
| Batch order | two screenshot items and one non-screenshot item produce association/image pairs in input order with original indices | `tests/batch_fetch_retrieval.rs` |
| Batch failures | failed and non-screenshot items emit no image or association; order and telemetry unchanged | `tests/batch_fetch_retrieval.rs` |
| Offload | screenshot batch is not task-offloaded; non-screenshot batch behaviour unchanged | `src/mcp/server.rs` unit tests, `tests/mcp_tools.rs` |
| Cache | `cache_policy=default` still captures; no screenshot cache entry; no PNG/base64 in cache-visible structures | `tests/web_fetch_integration.rs` |
| Schema | `include_screenshot` advertised in full and compact schemas for `web_fetch` and batch web items | `tests/mcp_schema_slimming.rs` |
| Pinned surface | fingerprints and bytes re-pinned deliberately; budget measured | `tests/mcp_tool_contract.rs`, `tests/tool_surface_live.rs` |

Fixtures use a synthetic PNG builder (signature + IHDR + padding); no decoder and no live browser are required. Tests that exercise a successful capture install a `BrowserLifecycle` whose synthetic `BrowserDiscovery` reports available, plus the override, on a per-test `ServerState`; capability-matrix tests leave the lifecycle absent or unavailable. No process-global mutable state is introduced.

## Compatibility and protocol effects

- Additive request field, default false; omitted calls are byte-identical.
- Structured JSON and output schemas are unchanged. Opted-in calls gain content blocks only.
- The progress/outcome runners change return type; the JSON-only `run_web_fetch`/`run_batch_fetch` wrappers keep their signatures, and two internal test call sites switch to `.value`.
- No new tool, no rename, no stable-ID or ordering change.
- Screenshot batches run synchronously; this is the documented tradeoff for native image content.
- No cache-format or cache-key change. `renderer_version` is not bumped because extracted text is unchanged.
- Schema size grows by the new boolean fields; the pinned advertisements and, if needed, the budget are updated with measured values and a recorded reason, never by silently dropping descriptions.

## Verification

- `cargo test --locked --features 'mock browser' --test browser_transport --test web_fetch_integration`
- `cargo test --locked --features 'mock pdf browser' --test batch_fetch_retrieval`
- `cargo test --locked --all-features --test mcp_tools --test mcp_tool_contract --test mcp_2026_protocol --test mcp_projection --test static_guards --test docs_tool_names --test mcp_schema_slimming --test tool_surface_live`
- `cargo test --locked --no-default-features --features mock --test web_fetch_integration --test batch_fetch_retrieval`
- `make check`; `make docs-check`

Deterministic mocks and fixtures are mandatory. Live browser smoke is optional operational evidence and cannot replace the keyless gate. Record unavailable-browser and cap-saturation cases explicitly.

## Documentation and static guards

- `architecture/fetch.md`: screenshot trigger, capture point, bounds, error and cache semantics.
- `architecture/mcp.md` and `docs/tool-matrix.md`: opt-in field, image-content association, batch ordering, and synchronous eligibility.
- `docs/features.md`: webpage-only scope and browser requirement; no PDF screenshots.
- `docs/test-inventory.md` and `architecture/testing.md`: new test counts and placement.
- `CHANGELOG.md`: the additive field and image-content behavior.
- Keep all ten-tool, schema, browser SSRF/lifecycle, and module-size guards green; re-pin only measured public-surface values.

## Acceptance and stop conditions

- `include_screenshot` absent or false leaves arguments, structured JSON, projection, and behavior unchanged.
- True returns a bounded viewport PNG as native MCP image content only after an actual browser render; capability failures are explicit.
- Dimensions and bytes are exercised at, below, and above each cap; no image bytes appear in JSON, caches, logs, progress, or diagnostics.
- Single-fetch association is unambiguous; batch association names the correct original index and label in input order, and failed/non-screenshot items emit nothing.
- No screenshot is produced for PDF, repo, or local workspace targets.
- Stop if the pinned MCP implementation cannot serialize structured JSON and image content together, if association is ambiguous, if bounds cannot be enforced before assembly, or if security requires weakening browser policy. Return for a revised ADR rather than switching to Base64-in-JSON or filesystem paths.

## Risks and mitigations

- **Target activation:** `Page::screenshot` calls `Target.activateTarget`; if it fails in headless multi-context use, the request fails explicitly. Live browser smoke remains unrun and is recorded as a limitation.
- **Memory and payload:** the browser materializes the encoded PNG before validation; the 4 MiB cap bounds the artifact and the ~5.6 MiB base64 string. The plan keeps validation immediately after capture and before assembly.
- **Mock masking production wiring:** the override replaces only the transport call; concept capture, validation, association, and assembly stay on the production path. Live smoke remains required operational evidence.
- **Content-type detection:** non-webpage skip relies on the captured main-response content type; if the event is unavailable, current code defaults to `text/html` and capture proceeds. Record this as a known limitation rather than adding a second request.
- **Schema growth:** the two boolean fields are small, but the exact increase is measured before re-pinning.

## Closure evidence

Recorded in `plans/closure/adaptive-fetch-routing/003-status.md` with the exact candidate SHA, accepted ADR, screenshot bounds, capability and behavior evidence, schema pins, checks, documentation, and unrun operational gates.
