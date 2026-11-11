# M001 — PDF Fetch Contract and Budgets

Status: proposed
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M001
Primary class: invariant (corrective)
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `architecture/fetch.md` § 6–7; `src/fetch/client.rs`; `src/fetch/pdf.rs`; `src/fetch/detect.rs`; `src/fetch/limits.rs`; `src/core/config.rs`; `src/mcp/tools/provider_status.rs`; `docs/config.md`; `docs/features.md`.
Dependencies: none. Not gated on ADR-0007 or ADR-0008 — the fetch contract must be correct under any availability or extraction decision.

## Objective

Make the PDF fetch boundary correct and honest before any extraction-quality work changes it. Concretely: give PDFs a byte budget separate from the generic response cap, stop reporting a size problem as a parse error, classify an image-only page as scanned without requiring `include_media`, stop failing `pdf_ocr: auto` on documents that need no OCR, and make the compiled-out refusal actionable.

## Non-goals

- No extraction algorithm change: reading order, line assembly, ligature handling, columns, and tables belong to M003 and M004.
- No chunking, focus, or payload change: that is M005.
- No change to the default feature set or `pdf_enabled` default. Availability is ADR-0007 and M006.
- No page-count default change and no new page-spec syntax.
- No new MCP tool, argument, or renamed wire string.

## Current implementation evidence

- `[fetch].max_bytes` defaults to 2,000,000 and is the only body cap applied to PDFs. A 2,215,244-byte arXiv paper with a `Content-Length` is refused before download; a 2,858,365-byte chunked response is silently truncated and then surfaces as `pdf_parse_error` with a cross-reference failure, which attributes a size problem to the document. Reproduced by truncating the document to the cap and re-extracting.
- `classify_page_quality` receives `has_images`, but `has_images` derives from `count_page_images`, which only runs when `include_media` is true. An image-only document therefore classifies as `Blank` and fails with `pdf_no_extractable_text` at defaults; `ScannedOrImageOnly`, the 0.10 quality score, and the "appears scanned or image-only" warning appear only when the caller sets `include_media`.
- `extract_pdf_text` returns `PdfOcrUnavailable` before extracting any page whenever the policy is `Auto` or `Always`, so `pdf_ocr: "auto"` cannot succeed even on a fully text-based document.
- `detect_from_content_type` has no `application/pdf` arm; only the `.pdf` path extension maps to `DocumentKind::Pdf`. The PDF branch currently returns before the document classifier runs, so this is latent rather than user-visible.
- `pdf_not_compiled_in` renders as "enable the `pdf` Cargo feature" with no build or documentation pointer.

## Invariants that cannot regress

- Response byte bounds, redirect policy, SSRF validation, timeouts, and deadlines apply to PDFs exactly as to other content; a PDF-specific budget is still a bound.
- Per-page characters, total characters, page count, outline entries and depth, and page-label expansion caps are unchanged.
- Sanitization, control-character stripping, trust markers, and the `untrusted` warning are unchanged.
- `FetchErrorKind` wire strings are unchanged unless a new value is explicitly additive and documented in the same change. Prefer reuse.
- Cached derived documents embed page metadata and classification, so this milestone changes cached content even though it does not change extracted text. It **must** bump the derived cache `renderer_version` from 2 with an isolation test, rather than serving a pre-change entry whose page carried the old `Blank` or OCR classification.

## Expected production changes

1. **PDF byte budget.** New `[fetch].pdf_max_bytes` with a default large enough for ordinary documents (proposed 16 MiB) and a validation range that still bounds memory (proposed 1 byte to 256 MiB). The effective body cap is `pdf_max_bytes` when the response is PDF-identified by Content-Type or `.pdf` path, and `max_bytes` otherwise. Magic-sniffed PDFs that reach the body read without either signal stay under `max_bytes`; document that limitation rather than widening the generic cap.
2. **Size-versus-parse reporting.** When a PDF body is refused for declared length or truncated at the effective cap, return the existing size outcome naming the configured value. Do not attempt parse and do not translate the failure into `pdf_parse_error`.
3. **Scanned classification without `include_media`.** Always determine image-XObject presence for quality classification; keep `include_media` as the switch that decides whether image counts are *reported* in page metadata. Preserve the existing page-metadata shape.
4. **OCR policy semantics.** `auto` attempts text extraction and records scanned/OCR-unavailable outcomes per page instead of failing the document; `always` keeps failing closed with `pdf_ocr_unavailable` because it demands a capability the build does not have.
5. **Document classifier coherence.** Add `application/pdf` to content-type detection with a unit test, so the second classifier cannot disagree with the fetch gate.
6. **Actionable refusal.** Keep the `pdf_not_compiled_in` kind and change the message to name the feature, the documentation anchor, and — if ADR-0007 is accepted later — the install-relevant guidance. Keep `provider_status.pdf_capabilities` consistent with feature, config, and budget state.
7. **Cache invalidation.** Bump `renderer_version` and add the isolation test, because page classification is part of the cached derived document.

## Storage, protocol, and compatibility effects

- One additive config field with a documented default and range; existing configs keep working and keep their current effective cap when `pdf_max_bytes` is absent.
- Additive classification values only. `pdf_no_extractable_text` still occurs for documents with no text layer; the difference is that image-only pages now say why.
- Response shape unchanged. `include_media` keeps its documented meaning for reporting.
- No migration. Operators who set `max_bytes` below `pdf_max_bytes` keep the smaller cap for non-PDF content only.
- Derived cache entries invalidate through the `renderer_version` bump, because cached documents embed the page classification this milestone changes.

## Ordered work packages

1. Add the config field, default, validation, and the effective-cap selector at the response read boundary; add unit and config tests.
2. Route PDF refusal and truncation to the size outcome; add tests for declared-length refusal, chunked truncation, and the non-PDF path staying unchanged.
3. Decouple image detection from `include_media` in `classify_page_quality`; add the image-only and blank-page tests.
4. Change `Auto` OCR handling to per-page outcomes; keep `Always` failing closed; add both tests.
5. Add the `application/pdf` classifier arm and its test.
6. Update the refusal message and `provider_status` consistency; assert both.
7. Bump `renderer_version` and add the cache-isolation test.
8. Update `docs/config.md`, `docs/features.md`, `architecture/fetch.md` § 6–7, and `architecture/config.md`.

## Verification

- `cargo test --locked --features pdf --test web_fetch_integration`
- `cargo test --locked --all-features --lib fetch::pdf::`
- `cargo test --locked --all-features --test config_validation`
- `cargo test --locked --all-features --test fetch_safety --test adversarial_corpus`
- `cargo test --locked --all-features --test mcp_tools --test provider_routing`
- `cargo test --locked --all-features --test docs_config_snippets`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `docs/config.md` gains the field with its default and range; `docs/features.md` states the budget and the reported-versus-attempted OCR behaviour.
- `architecture/fetch.md` § 7 owns the gate description and the two-classifier relationship; `architecture/config.md` owns the validation rule.
- `tests/docs_config_snippets` must stay green; do not add an unverified config snippet.
- No new test suite is required; the behavioral tests extend `web_fetch_integration` per `architecture/testing.md`.

## Acceptance and stop conditions

- A 2.2 MB PDF with a `Content-Length` is fetched and extracted under the default configuration.
- A PDF above `pdf_max_bytes` returns the size outcome naming the configured value, not `pdf_parse_error`.
- A chunked PDF truncated at the cap returns the same size outcome.
- Non-PDF content above `max_bytes` still fails exactly as today.
- An image-only PDF without `include_media` reports `ScannedOrImageOnly` with the scanned warning; `include_media` changes reported counts, not classification.
- `pdf_ocr: "auto"` extracts a text PDF; `pdf_ocr: "always"` still returns `pdf_ocr_unavailable`.
- `renderer_version` is bumped and a pre-bump cached document is demonstrably not served.
- No wire string is renamed and `tests/docs_tool_names` is untouched and green.
- Stop if the change requires altering non-PDF byte-cap semantics, a new MCP argument, or a renamed error kind. Stop and escalate if a new wire value appears unavoidable.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/001-status.md` records the candidate SHA, the cap-selection matrix with the observed outcome kinds, the before/after scanned-classification results on an image-only document, the OCR-policy matrix, the config validation cases, and the `renderer_version` bump with its isolation-test output.
