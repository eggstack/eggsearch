# M001 — Target-Aware Fetch Routing

Status: active
Source: `plans/subsystems/adaptive-fetch-routing-roadmap.md` M001
Primary class: capability
Baseline: `b7f7497` (`feat/auto-20261117-044c4885`)
References: `plans/000-long-term-specification.md` §§1–4; `plans/001-terminology-and-domain-model.md` §4–5; accepted ADR-0001, ADR-0002, ADR-0007, ADR-0008, ADR-0014; `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md`; `plans/implementation/pdf-ingestion-fidelity/008-local-workspace-and-batch-ingestion.md`; `architecture/fetch.md`; `architecture/local-workspace.md`; `src/core/fetch_locator.rs`; `src/mcp/tools/web_fetch.rs`; `src/mcp/tools/repo_fetch.rs`; `src/fetch/client.rs`.
Dependencies: interface dependency on the written PDF M001 budget/outcome contract and ADR-0014 browser behavior. Soft integration dependency on PDF M008: the route may be developed against its stable root-relative workspace/PDF interface, but local-PDF end-to-end acceptance must run against the landed M008 behavior.

## Objective

Make one internal, typed target-resolution path dispatch explicit web URLs, remote repository locators, and local workspace locators to their owning fetch behavior. For web URLs, preserve the existing content-detection contract: PDF responses use bounded PDF extraction, HTML/text uses the existing renderer, and browser rendering follows the existing explicit or automatic policy. Keep `web_fetch`, `repo_fetch`, and `batch_fetch` as separate public tools; do not add a general-purpose fetch tool.

This milestone integrates existing capability rather than replacing extraction algorithms. The PDF M008 plan remains the owner of local PDF ingestion; this milestone consumes that interface and must not create a second local file reader.

## Non-goals

- No new MCP tool, renamed tool, or new arbitrary target string/URL scheme.
- No screenshot request or image output; that is M003 and requires accepted ADR-0015.
- No changes to ordinary browser `auto` escalation criteria, browser lifecycle, profiles, or challenge behavior.
- No PDF rasterization, OCR, page image extraction, or change to the text-only PDF strategy.
- No absolute local paths, `file://`, recursive discovery, or weakening of local workspace authorization.
- No changes to PDF inclusion in code indexing, symbol parsing, or repo maps.
- No broad cache, transport, or extraction rewrite.

## Current implementation evidence

- `src/fetch/client.rs` performs bounded HTTP fetch and derives extraction from response content; `architecture/fetch.md` specifies header/extension detection, `%PDF-` post-body sniffing, HTML rendering, PDF extraction, and browser rendering as a separate escalation.
- `src/mcp/tools/web_fetch.rs` resolves `render` and PDF options before invoking the shared client. `RenderPolicy::Auto` is not a request to render every page.
- `src/core/fetch_locator.rs` defines typed target resolution for web URLs, remote repositories, and workspace locators. The current worktree wires web/repo wrappers and batch preflight through that resolver; owning handlers still perform their own transport and authorization checks.
- `src/mcp/tools/repo_fetch.rs` builds remote source URLs for repo targets. For `host: "workspace"`, it resolves an enabled root and validates a root-relative path before `safe_read_file`; a fixture-backed regression preserves a detected remote-PDF document outcome rather than decoding it as source.
- PDF M008 is already a ready, bounded plan for local workspace PDF extraction and batch web page selection; it explicitly keeps PDFs out of code indexing and does not add arbitrary filesystem targets.

## Invariants that cannot regress

- Exactly ten registered MCP tools and current public tool names.
- Every web URL remains subject to `FetchClient` URL validation, DNS/address pinning, SSRF and redirect policy, byte/deadline limits, cache isolation, and sanitization.
- PDF classification and its four outcomes continue to follow the existing PDF contract; a failed or unavailable PDF never falls through to UTF-8 source decoding or browser rendering.
- HTML extraction and browser escalation preserve the accepted ADR-0014 policy and current lifecycle bounds.
- Remote repo locators keep repo-fetch trust, line/span, permalink, and error semantics.
- Local locators contain only an authorized workspace root name and root-relative path; traversal, absolute paths, hidden-file policy, symlink policy, containment, and safe-read byte caps remain enforced.
- Stable IDs remain content-derived and deterministic. Routing alone does not change renderer/cache identity.

## Expected production changes

1. Establish a single internal target-resolution contract using the existing locator types (or a narrowly extracted equivalent): web URL, remote repo, and local workspace. Keep input validation target-specific and reject mismatched fields rather than guessing a route.
2. Route web URLs to the shared bounded `FetchClient`; retain content-type/path preflight and magic sniffing as the source-selection evidence. PDF extraction, HTML/text rendering, and browser escalation remain separate downstream paths with their existing capability errors.
3. Route remote repo and workspace targets through repo-fetch semantics. A remote repo raw fetch identified as PDF retains its PDF document outcome and is never decoded as source code; it does not become code-span evidence or enter indexing. Workspace PDF behavior is delegated to PDF M008 once landed; do not duplicate its size budget, path validation, extraction outcome mapping, or `FetchDocument` construction.
4. Keep the public wrappers responsible for their existing response shapes and projection. Do not expose internal `FetchLocator` variants as a new generic MCP schema.
5. If an extraction output changes, apply the existing renderer-version and corpus-measurement rules; do not bump `renderer_version` for dispatch-only refactoring.

## Ordered work packages

1. Add a target-resolution table and deterministic tests for web URL, remote repository, workspace root/path, invalid URL scheme, absolute path, traversal, and unknown workspace root.
2. Wire the single-target wrappers to the shared internal resolver without moving transport policy or changing their request/response contracts.
3. Prove the web route matrix: PDF by Content-Type, extension, and `%PDF-` sniff; HTML/text through existing renderers; explicit browser request; automatic browser escalation only for the existing conditions; browser unavailable/disabled errors remain explicit.
4. Prove remote repo and workspace routing retain their current trust, line/span, path-safety, and failure semantics. Once PDF M008 is implemented, add the local PDF route to the integration matrix without duplicating its handler.
5. Update owning architecture and feature docs only where observed behavior changed; update suite inventory counts if tests are added to existing behavioral suites.

## Verification

- `cargo test --locked --features 'mock pdf browser' --test web_fetch_integration --test extract_fetch_contract --test fetch_safety`
- `cargo test --locked --no-default-features --features mock --test web_fetch_integration --test extract_fetch_contract --test fetch_safety`
- `cargo test --locked --features 'mock pdf' --test local_workspace_integration` once PDF M008 implementation is present; otherwise record the integration case as blocked, not passed.
- `cargo test --locked --all-features --test mcp_tools --test static_guards --test docs_tool_names`
- `make check`; `make docs-check`

Tests must use committed fixtures/mocks and remain keyless and network-free. `make check` is the broad gate, not a substitute for the route-specific matrix.

## Documentation and static guards

- Update `architecture/fetch.md` for the shared target-to-handler contract and preserve its existing transport/extraction ownership.
- Update `architecture/local-workspace.md` only if the target-resolution entry point changes; PDF M008 owns detailed local-PDF configuration and behavior.
- Update `architecture/mcp.md`, `docs/tool-matrix.md`, and `docs/features.md` only if public behavior or schema actually changes. No tool inventory count change is expected.
- Keep `tests/docs_tool_names.rs`, stable tool registration guards, local path safety guards, and `tests/static_guards.rs` green and unchanged in intent.

## Acceptance and stop conditions

- A valid HTTP(S) URL reaches the bounded web-fetch path and routes PDF, HTML/text, and browser cases as the existing contracts specify.
- Remote repo locators retain repo-fetch semantics; workspace locators resolve only through enabled authorized roots and safe root-relative reads.
- No `file://`, absolute local path, traversal, or fallback from a failed PDF extraction becomes a successful alternate route.
- No public tool is added, removed, renamed, or collapsed.
- Local PDF acceptance is recorded only after PDF M008's supported/over-budget/feature-disabled cases are exercised on the integrated candidate.
- Stop if correct routing requires weakening ADR-0008, changing ADR-0014's ordinary auto behavior, or expanding the ten-tool inventory; return for a new decision rather than silently modifying contracts.

## Closure evidence

Write `plans/closure/adaptive-fetch-routing/001-status.md` with the candidate SHA, target-to-handler matrix, PDF/HTML/browser outcomes, repo/workspace security matrix, feature-absent behavior, direct and cached path results, tool/schema inventory, focused/broad checks, documentation, and unresolved or blocked evidence (including any not-yet-landed PDF M008 integration).
