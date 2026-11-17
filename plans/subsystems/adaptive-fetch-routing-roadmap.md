# Adaptive Fetch Target Routing and Artifact Roadmap

Status: active (M001 active; M002 closed; M003 ready; ADR-0015 accepted)
Planning baseline: `b7f7497` (`feat/auto-20261117-044c4885`, after rebase onto `origin/main`)
Owner: fetch/extraction and MCP orchestration

References: `plans/000-long-term-specification.md` §§1–4, 6; `plans/001-terminology-and-domain-model.md` §4–5; `plans/002-long-term-roadmap.md`; `plans/003-planning-process.md`; accepted ADR-0001, ADR-0002, ADR-0007, ADR-0008, ADR-0014, and ADR-0015.

## 1. Purpose and ownership boundary

Make target selection predictable across URL, remote repository, and local workspace fetches without collapsing their distinct safety and result semantics. A web URL is boundedly fetched and classified as PDF, HTML/text, or another supported representation; PDF extraction and HTML extraction use their existing pipelines, and browser rendering remains the explicit or policy-driven escalation for webpages. Repository locators continue through repo-fetch semantics. Workspace files, including local PDFs after PDF M008 lands, remain root-relative and authorized by the local-workspace boundary.

This workstream owns the internal target-resolution and fetch orchestration seams used by `web_fetch`, `repo_fetch`, and `batch_fetch`, plus parity for applicable per-item target controls. It does not add a general-purpose MCP fetch tool. The public tool names and the ten-tool contract remain unchanged.

The screenshot capability is a separate, gated milestone because it changes MCP result content and is in tension with accepted PDF/browser decisions. It is scoped to explicitly requested HTTP(S) webpage screenshots; it is not a PDF image pipeline.

## 2. Work classification

- **Invariant:** exactly ten tools; typed web/repo/workspace locator semantics; bounded and SSRF-safe network access; root-relative local reads; stable IDs, order, trust framing, cache isolation, and current browser lifecycle/security limits.
- **Capability:** target-appropriate PDF extraction, HTML extraction, or browser rendering; per-target batch parity; opt-in webpage screenshot artifacts under accepted ADR-0015 after M002 closes.
- **Infrastructure:** a shared internal target resolver/execution seam and image-content result assembly.
- **Polish:** diagnostics and documentation that describe actual target capabilities and outcomes.

## 3. Current implementation evidence

- `src/fetch/client.rs` and `architecture/fetch.md` document bounded URL fetching, content-type and extension checks, post-body `%PDF-` sniffing, PDF extraction, HTML rendering, and the existing browser `RenderPolicy::Auto` escalation. Ordinary automatic rendering is not a screenshot request and remains governed by ADR-0014.
- `src/mcp/tools/web_fetch.rs` exposes `pdf`, `render`, and `browser_profile` options. M002 adds those controls to `BatchFetchItem::Web`, adds repo source-selection controls to `BatchFetchItem::Repo`, and routes web items through the shared web-fetch handler.
- `src/mcp/tools/batch_fetch_item.rs` routes repo items through `run_repo_fetch_with_host_scope`; the server wrapper forwards the same private `codegg/host-local-scope` metadata used by `repo_fetch`. Batch preflight validates target-specific controls before item execution.
- `src/core/fetch_locator.rs` defines `FetchLocator::{WebUrl, Repo, Local}` and maps workspace repo items to a root name plus relative path. It does not create a public generic tool.
- `repo_fetch` handles `host: "workspace"` by resolving an enabled authorized root, validating a relative path, and using `safe_read_file`. PDF M008 is already registered and ready; it owns local PDF extraction and batch web page selection. It explicitly leaves PDF files in the binary/indexing exclusion set and does not authorize absolute paths or `file://` URLs.
- No webpage screenshot capture or MCP image-content assembly exists in `src/`. The common MCP result conversion currently produces structured JSON.

## 4. Invariants and non-goals

- Keep `web_fetch`, `repo_fetch`, and `batch_fetch` as separate public tools; do not add a generic `fetch` tool or alter tool names.
- Preserve HTTP transport ownership, SSRF checks, address pinning, redirects, deadlines, bounded bodies, sanitization, cache scope, and current browser lifecycle controls.
- Use content type, extension, and magic detection as the existing contract requires; do not select PDF extraction from an unchecked filename alone when the body contradicts it.
- Do not render PDFs in a browser. PDF extraction remains deterministic, bounded, text-only, and governed by ADR-0008.
- Local targets are workspace-root-relative locators resolved under the same authorization, containment, symlink, hidden-file, and byte-cap rules as repo workspace fetches. No arbitrary absolute path or `file://` input.
- Keep PDFs out of code indexing, symbol parsing, and repo maps.
- No recursive crawling, OCR, page-image extraction, browser interaction, challenge solving, or new binary SKU.
- Screenshots remain opt-in and web-only. M003 is not ready until M002 closes; accepted ADR-0015 may not silently override ADR-0008 or ADR-0014.

## 5. Target and handler contract

| Target | Resolution | Content behavior | Safety boundary |
|---|---|---|---|
| Web URL | Explicit HTTP(S) URL | Bounded fetch; PDF by headers/path/magic uses the PDF extractor; HTML/text uses the existing renderer; browser rendering uses existing explicit/auto policy, except an accepted screenshot request explicitly asks for browser capture. | `FetchClient` URL validation, SSRF, redirect, byte and deadline limits. |
| Remote repository | Structured host/owner/repo/ref/path locator | Text source follows `repo_fetch` raw-source and line/span semantics. If the bounded raw fetch identifies a PDF, retain the existing PDF document outcome rather than decoding it as source code; PDF remains excluded from indexing, symbols, and repo maps. | Existing host and path validation, fetch policy, bounded transport. |
| Local workspace | Workspace root name plus root-relative path | `repo_fetch` workspace semantics; local PDF extraction comes from PDF M008, not a second local reader. | Enabled/authorized root, centralized path validation, safe bounded open/read. |

The shared internal resolver must route to the owning handler and preserve its result/error shape. It is not a replacement for target-specific argument types.

## 6. Dependency graph and milestones

```text
PDF roadmap M001 budget/outcome contract ── interface ──> M001 target routing
PDF roadmap M008 local-PDF/batch contract ── soft/interface ──> M001, M002
M001 target-routing contract ── interface ──> M002 batch parity
M002 batch parity ── hard ──> M003 screenshot output
ADR-0015 accepted ── hard ──> M003 screenshot output
ADR-0014 accepted browser contract ── interface ──> M001, M003
```

| Milestone | Class | Status | Handoff | Dependencies | User-visible exit |
|---|---|---|---|---|---|
| M001 — Target-aware fetch routing | capability | active | `plans/implementation/adaptive-fetch-routing/001-target-aware-fetch-routing.md` | Interface: PDF M001 budget/outcome contract and ADR-0014. Soft integration dependency: PDF M008 for local-PDF end-to-end coverage. Implementation is committed on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; its closure record remains outstanding. | A URL or typed repo/workspace target reaches the correct existing bounded handler; PDF, HTML, and browser outcomes are explicit and do not cross target boundaries. |
| M002 — Batch per-target capability parity | capability | closed | `plans/implementation/adaptive-fetch-routing/002-batch-per-target-capability-parity.md` | Interface: M001 target contract and PDF M008 batch/local-PDF contract. Closed on candidate `24f045729f8409dadc96b4c2455253d8a9c7b6ec`; `make check` passed with 6,347 tests and the closure record is `plans/closure/adaptive-fetch-routing/002-status.md`. | Applicable URL and repo/workspace target controls work per item, with single-target semantics and preserved batch order, errors, IDs, budgets, and telemetry. |
| M003 — Opt-in webpage screenshots | capability | ready | `plans/implementation/adaptive-fetch-routing/003-opt-in-webpage-screenshot-artifacts.md` | M002 is closed and ADR-0015 is accepted. Serializer/transport and browser-fidelity validation remain implementation gates; the seam and fixture research is recorded in the plan. Interface: ADR-0014 browser contract. | An explicit request returns a bounded webpage screenshot as MCP image content; absent/false preserves the existing response. |

M008 remains the implementation owner for local PDF extraction and `batch_fetch` per-URL PDF page selection. This roadmap consumes its stable interface and must not amend or duplicate that scope. M002 is closed, so M003 is ready; accepted ADR-0015 fixes the result contract and narrow ADR supersession, and M003 implementation research is recorded in its plan.

## 7. Cross-cutting concerns

- **Security:** URL targets continue through the existing dynamic-fetch SSRF and redirect pipeline. Workspace paths never bypass the authorized-root and safe-open boundary. Screenshots run only in the existing bounded browser context, without interaction or policy relaxation.
- **Data handling:** fetched text remains external-untrusted; local text retains repo workspace trust semantics. Screenshot artifacts are external, explicitly requested visual content, bounded and not written to cache or diagnostics.
- **Cache:** content/extraction options remain part of derived cache identity. A routing-only refactor does not bump `renderer_version`; any extraction-output change requires the existing version discipline. Accepted ADR-0015 prohibits screenshot caching.
- **Protocol:** additive request/result details only where approved; no new tool and no change to stable IDs, result order, or error strings. Screenshot content is a narrowly gated exception controlled by ADR-0015.
- **Documentation:** update `architecture/fetch.md`, `architecture/local-workspace.md`, `architecture/mcp.md`, `docs/tool-matrix.md`, `docs/features.md`, `docs/config.md` if configuration changes, and `docs/test-inventory.md` when suite counts change. Keep `AGENTS.md` as an index.

## 8. Risks and deferred work

The main correctness risk is divergence between the single-fetch and batch implementations, especially cache keys, profile scope, and PDF options. The common resolver and direct-vs-batch equivalence tests are the mitigation. The local PDF path has an existing planned owner (PDF M008); if its contract changes, this roadmap must reconcile before integration rather than implementing another path.

Static API feasibility is established, but screenshot capture, serializer/transport behavior, and client association remain untested until M003 implementation. Screenshot capture adds substantial browser cost and can expose authenticated page content; the explicit per-call opt-in, current profile-origin checks, accepted strict bounds, and no caching/logging are required. If image content cannot be represented and associated with batch items in the pinned MCP implementation, stop M003 and return for a new contract decision.

Deferred: new MCP tools, target discovery/crawling, PDF rasterization/OCR, screenshot caching, full-page screenshots, image analysis, browser interaction, and changing the browser default or escalation policy.

## 9. Closure and status

M002 is closed on `24f045729f8409dadc96b4c2455253d8a9c7b6ec`: `make check` passed with 85 suites and 6,347 tests, `make docs-check` passed, and the closure record records the acceptance matrix, commands, qualifications, and limitations. M001 remains active on the same implementation because its own closure record has not been written. The earlier default-parallel Startpage failure was a test-isolation defect now fixed; the `RUSTSEC-2026-0543` chrono notice carries a scoped review-dated waiver; and the fake-Plan9 installer fixture now uses a hermetic PATH. M003 is ready: ADR-0015 is accepted, and the implementation research in its plan identifies the result-assembly seam, capture point, bounds enforcement, test-infrastructure gap, and open decisions. No hosted CI, push, or live-browser qualification ran on this candidate.

Each milestone requires `plans/closure/adaptive-fetch-routing/NNN-status.md` with candidate SHA, target-to-handler matrix, direct-vs-batch parity evidence, focused and broad checks, unchanged safety/cache/tool-surface evidence, documentation, limitations, and unrun gates. M003 additionally records MCP image association, numeric image bounds, cache behavior, and explicit ADR compliance.
