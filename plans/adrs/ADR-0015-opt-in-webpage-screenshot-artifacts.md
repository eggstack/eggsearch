# ADR-0015: Opt-In Webpage Screenshot Artifacts

Status: accepted
Date: 2026-11-17
Accepted: 2026-11-17
Decision owners: project maintainers
Affected subsystems: fetch and extraction; browser rendering; MCP result content; batch fetch.
Related: `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md`; `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`; `plans/subsystems/adaptive-fetch-routing-roadmap.md`.

## Context and forces

The adaptive-fetch workstream needs an explicitly opt-in way to return a visual record of a webpage. Eggsearch already has bounded browser rendering and an automatic escalation policy, but it does not capture screenshots. Current MCP success conversion returns structured JSON with a text fallback; there is no screenshot path in the tool result or batch item contract.

Two accepted decisions constrain this work. ADR-0008 selects text-only PDF extraction and says no image bytes leave the process. ADR-0014 keeps the existing tool arguments and response shapes unchanged while defining browser availability and lifecycle bounds. A webpage screenshot therefore needs an explicit, narrow decision; it must not be introduced as an incidental side effect of browser rendering or treated as permission to rasterize PDFs.

Forces include preserving the ten-tool surface, keeping screenshots out of unbounded JSON text, associating batch screenshots unambiguously with their items, preventing stale or cross-profile image reuse, and retaining browser SSRF, origin, timeout, and lifecycle controls.

## Considered alternatives

1. **Add Base64 image bytes to the structured JSON response.** Rejected as the proposed default: it expands the text/structured payload substantially, complicates response budgeting and projection, and makes batch output especially expensive.
2. **Return a temporary filesystem path or an out-of-band artifact URL.** Rejected: it creates a new path/transport lifecycle, can be inaccessible to remote MCP clients, and would need separate authorization, expiry, and cleanup semantics.
3. **Return a native MCP image content block alongside the existing structured result.** **Selected.** It keeps image data in the protocol's image content channel while retaining a compact structured result. Batch image blocks can be emitted in input order with an explicit item index and label.

## Decision

1. Add an optional `include_screenshot` boolean to `web_fetch` and to each `batch_fetch` web item. Omitted or `false` preserves today's behavior and response content.
2. `include_screenshot = true` is an explicit request for browser rendering and capture, including for pages that would otherwise remain on the HTTP path. It may override `render = auto`; an explicit contradictory `render = http_only` is a validation error. If browser support is unavailable or disabled, return the existing actionable browser capability error; never silently return an HTTP-only result without a screenshot.
3. Capture one bounded viewport PNG after navigation/render completion, with no page interaction, scrolling, OCR, or full-page rasterization. The hard pixel-dimension and encoded-byte caps below are part of this decision; the implementation milestone must enforce and test both limits before M003 can close. Existing browser network, origin, timeout, and process-lifecycle policy remains in force.
4. Return image bytes as a native MCP image content block. Keep structured JSON free of image bytes; include only bounded screenshot metadata needed to associate the artifact. For `batch_fetch`, emit a text association and image block for each successful screenshot in original item order, labelled with that item's index and label. Failed or non-screenshot items emit no image block.
5. Generate screenshots only for HTTP(S) webpage targets. Do not screenshot PDFs, remote repository files, or local workspace files. The PDF extractor remains text-only: no PDF rendering, rasterization, OCR, or image extraction is authorized.
6. Do not store screenshot bytes in the raw or derived fetch cache, logs, traces, or diagnostics. A screenshot request must produce a new browser capture; text/cache status retains its existing meaning. A supplied browser profile continues to be subject to its existing origin authorization and locking rules; screenshot capture adds no login or interaction capability.

This accepted ADR makes a narrow exception for explicitly requested webpage screenshots to ADR-0008's statement that no image bytes leave the process; ADR-0008's PDF-specific text-only strategy remains unchanged. It also supersedes only ADR-0014's no-response-shape-change constraint for calls that explicitly opt into screenshots. ADR-0014's default browser policy, escalation criteria for ordinary calls, security controls, and lifecycle bounds remain unchanged. No tool is added or renamed.

## Consequences and tradeoffs

The default response remains byte-for-byte unaffected by the new option when it is absent or false. An opted-in response gains MCP image content and bounded metadata, so clients that do not display image blocks may still read the structured fetch result but will not see the screenshot. Batch clients receive image blocks in addition to the existing per-item JSON results and must use the explicit index association.

An opted-in screenshot forces a browser launch and incurs the browser's CPU, memory, latency, and network costs even for static pages. Each invocation may observe authenticated page content when an already-authorized browser profile is selected; the explicit request, profile-origin checks, and absence of persistence in eggsearch's response cache are the controls. The screenshot is untrusted external content and must not be presented as verified or sanitized text.

## Compatibility, security, and migration

The request field is additive and defaults off. Existing structured JSON fields and error codes remain unchanged for callers that do not request a screenshot. The ten-tool inventory remains unchanged. Screenshot image blocks are returned only when requested and successfully captured; no hidden fallback, filesystem path, or data URL is introduced.

The implementation must keep screenshot capture within the browser's existing origin/network policy and request deadline, apply a strict byte and pixel cap, avoid writing image bytes to diagnostics, and include screenshot behavior in cache and response-projection tests. It must not weaken local workspace path authorization or permit `file://` access.

## Verification and status

This ADR was explicitly accepted on 2026-11-17 after the no-behavior-change feasibility preflight confirmed type-level MCP image/structured-content support, viewport-capture API feasibility, numeric bounds, and a batch association representation. The preflight did not test screenshot capture, serializer round-tripping, or live transport. Acceptance authorizes the contract, not implementation or browser fidelity; M003 remains proposed until its hard dependency M002 is closed. Exact-candidate tests and a closure record remain required.

## Feasibility preflight (2026-11-17)

The no-behavior-change static preflight supports the result and capture path selected by this decision:

- The pinned `rmcp` 3.9.0 `CallToolResult` exposes both `content: Vec<ContentBlock>` and `structured_content`; `ContentBlock` includes an `Image` variant. This is type-level support for carrying the existing structured result and image blocks together. A serializer/transport integration test is still required during implementation.
- The pinned `chromiumoxide` 0.9.1 exposes `Page::screenshot` returning `Vec<u8>`. `ScreenshotParams` wraps the CDP screenshot options and has a `full_page` control; `CaptureScreenshotParams` exposes `clip` and `capture_beyond_viewport`. A viewport-only capture is therefore expressible without adding a dependency. Since the returned bytes are materialized as a `Vec`, implementation must check the encoded size before MCP result assembly and keep capture within the existing browser deadline and lifecycle.
- Accepted hard capture caps: at most **1280 × 720 CSS pixels** at device scale factor 1 (**921,600 pixels**, or **3,686,400 bytes** for an RGBA pixel buffer), and at most **4,194,304 encoded PNG bytes** (4 MiB). Use a viewport capture, never full-page capture; reject any result exceeding either bound. The pixel cap bounds the principal image-buffer scale, while the encoded cap bounds the artifact passed to MCP.
- Batch association can use one bounded text content block immediately before each image block, naming the original input index and that item's existing label. Emitting each association/image pair in input order makes the association local and unambiguous without adding a tool or changing JSON schema.

This was static API feasibility only: no browser capture, serializer round-trip, or live transport test was run, and no production code or public schema changed. Following explicit maintainer acceptance on 2026-11-17, ADR-0015 is accepted; M003 production work remains gated on M002 closure. Acceptance authorizes the contract but does not supply implementation, serializer/transport, or browser-fidelity evidence.
