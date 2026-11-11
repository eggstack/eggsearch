# M008 — Local Workspace and Batch Ingestion

Status: ready
Source: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` M008
Primary class: capability
Baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731`
References: `plans/adrs/ADR-0007-pdf-artifact-availability.md` § Decision; `plans/adrs/ADR-0008-pdf-extraction-fidelity-strategy.md` § Decision; `plans/subsystems/pdf-ingestion-fidelity-roadmap.md` § 3, § 6, § 10; `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md` § Expected production changes; `src/core/local.rs`; `src/meta/safe_open.rs` § safe_read_file; `src/fetch/mod.rs`; `src/fetch/pdf.rs` § extract_pdf_text, § PdfExtractionResult, § PdfLimits; `src/fetch/client.rs`; `src/core/fetch.rs` § PdfFetchOptions; `src/core/repo_fetch.rs` § RepoFetchResponse; `src/core/batch_fetch.rs` § BatchFetchItem, § BatchFetchTelemetry; `src/mcp/tools/batch_fetch.rs`; `src/mcp/tools/repo_fetch.rs` § `run_workspace_fetch`; `tests/local_workspace_integration.rs`; `tests/batch_fetch_retrieval.rs`; `tests/config_validation.rs`; `tests/docs_tool_names.rs`; `tests/static_guards.rs`; `docs/tool-matrix.md`; `docs/features.md` § PDF Extraction; `docs/config.md` § Fetch Defaults, § Local Workspace Defaults; `architecture/local-workspace.md` § Configuration; `architecture/fetch.md` § 7. PDF Gate; `architecture/mcp.md` § The 10 Tools; `docs/test-inventory.md` § Behavioral & Workflow Contracts.
Dependencies: hard M001 (PDF fetch budget and classification). Soft M005: this milestone does not depend on chunking; if M005 has not landed it inherits today's single-chunk PDF shape and must say so rather than assume the newer one.

## Objective

Make the two ingestion surfaces that already accept explicit targets reach PDFs. A file under an enabled `[local]` root is readable through `repo_fetch` with `host: "workspace"`, and a `batch_fetch` web item can name pages with the same `pdf.pages` syntax `web_fetch` already uses. Both keep the existing bounds, resolve to one of the four document outcomes, and add no tool, no renamed argument, and no new page-syntax dialect.

## Non-goals

- No crawling or discovery: only an explicitly named local path or URL is considered.
- No indexing change: `pdf` stays in `BINARY_EXTENSIONS`, so code search, symbol parsing, and repo map keep skipping PDFs.
- No OCR, rasterization, rendering, image extraction, or attachment extraction.
- No page selection on `repo_fetch` (local or remote) and none on `batch_fetch` repo items; selection is per URL only.
- No new MCP tool, no renamed argument, no change to the existing page-spec syntax, and no change to `web_fetch` arguments or behaviour.
- No new response field for per-page metadata; classification is carried by existing warnings.

## Current implementation evidence

- `BINARY_EXTENSIONS` in `src/core/local.rs` contains `pdf`, and `is_binary_extension` lowercases the path extension before matching. `is_eligible_for_indexing`, the local-backend candidate filter, and `repo_mapper` all consult it, so PDFs are excluded from search and repo map today. `validate_local_fetch_path` rejects the same extension with `LocalFetchPathError::BinaryFile` after traversal and skip-directory checks; its unit tests assert both `.png` and uppercase `.PDF` rejection.
- `run_workspace_fetch` in `src/mcp/tools/repo_fetch.rs` is the only production caller of that validator. It reads through `safe_read_file` with `[local].max_file_bytes` (default 1,048,576) and then `String::from_utf8`-decodes the bytes.
- `safe_read_file` in `src/meta/safe_open.rs` reads at most `max_size` bytes into a `Vec<u8>` and returns `FileContentLimitExceeded` instead of over-allocating; the validator independently rejects the same limit as `FileTooLarge`.
- `run_workspace_fetch` builds `lines` with `apply_line_range` and `clamp_lines_to_max_chars`, builds `text` from those lines, and sets `document: None`. No local path constructs a `FetchDocument` or chunks, so this milestone adds document construction rather than reusing one.
- `extract_pdf_text(bytes: &[u8], max_chars, &PdfLimits, Option<&PdfExtractOptions>)` in `src/fetch/pdf.rs` is public and byte-slice based; `src/fetch/client.rs` already calls it with `&body`. The only seam is the `#[cfg(feature = "pdf")]` gate on `src/fetch/mod.rs`; the extractor needs no decoupling from the fetch response.
- `PdfExtractionResult` carries `document: FetchDocument` with `kind: DocumentKind::Pdf`, a legacy `text`, `warnings`, `structured_warnings`, `page_metadata`, and `quality_score`. `RepoFetchResponse` in `src/core/repo_fetch.rs` has `document: Option<FetchDocument>` but no page-metadata field.
- `PdfLimits` is filled from `Config::fetch_limits()`: 25 pages, 12,000 characters per page, 50,000 characters total.
- `web_fetch` exposes `pdf: Option<PdfFetchOptions>` with `pages`, `pdf_password`, `include_media`, and `pdf_ocr`, and feeds `pdf_pages` into the derived cache key. The page spec is parsed inside `FetchClient::derive_from_raw` after the body is read, so `web_fetch` has no pre-I/O page validation to copy.
- `BatchFetchItem::Web` carries `url`, `extract_mode`, `include_links`, `max_chars`, `cache_policy`, `max_cache_age_seconds`, `focus`, `focus_max_chunks`, and `focus_max_chars`, all optional with serde defaults; `Repo` has no URL, and `parse_batch_repo_host` maps `workspace` to the local handler.
- Batch pre-validates every effective item before it builds a client or spawns a wave; validation errors abort the call with an `item {i}` prefix, while per-item failures land in `results[index]` with `ok = false`. Results and telemetry are index-based, and `batch_fetch_id(label, index)` is independent of content.
- Every batch `build_derived_key` call passes `None, None, false` for `pdf_pages`, `pdf_ocr`, and `include_media`; without threading the item's options, a page-selected item would store or hit the default-extraction entry.
- `renderer_version` is the literal `2` inside `build_derived_key`, and `web_fetch` already keys on page selection. Local reads consult no cache at all.
- Docs that would go stale: `docs/tool-matrix.md` batch and `web_fetch` rows, `architecture/mcp.md` § The 10 Tools, `docs/features.md` § PDF Extraction, `docs/config.md` § Fetch Defaults and § Local Workspace Defaults, and `architecture/local-workspace.md` § Configuration.

## Invariants that cannot regress

- Exactly ten MCP tools; no argument renamed or removed; `web_fetch`'s `pdf` object and page syntax are untouched.
- `BINARY_EXTENSIONS`, `is_binary_extension`, `is_eligible_for_indexing`, and repo-map filtering are unchanged, so PDFs stay out of code search, symbols, and repo map.
- Local PDF bytes are read under an explicit cap through `safe_read_file`; no path may read an unbounded file into memory.
- Extracted PDF text passes through `strip_control_chars` and `scan_injection_markers`, is returned as `FetchTrust::LocalTrusted`, and gets no external-untrusted framing, identical to other local content.
- `FetchErrorKind` wire strings are unchanged; the four outcomes reuse `pdf_not_compiled_in`, `pdf_disabled`, `pdf_parse_error`/`pdf_encrypted`, and `pdf_no_extractable_text`.
- Batch input order, `index`, `label`, `stable_id`, `chars_returned`, and telemetry counters keep their meaning; page selection never enters `batch_fetch_id`.
- `renderer_version` stays 2, and identical bytes still produce identical extraction, IDs, and ordering.

## Expected production changes

1. **Local detection and path policy.** Treat a local file as a PDF when its extension is `pdf` (case-insensitive) or its first bytes are `%PDF-`; reuse `DocumentKind::Pdf` and add no classifier. Exempt PDF from the binary rejection in `validate_local_fetch_path` only; every other binary extension keeps `BinaryFile`. Update the uppercase-rejection unit test to use a non-PDF binary.
2. **Local size budget.** Add `[local].pdf_max_file_bytes` with `serde(default)`, default 16,777,216 bytes, and a validation range of 1..=268,435,456 in `Config::validate`'s `[local]` block. Align the default with M001's proposed `[fetch].pdf_max_bytes`; if M001 lands a different value, follow it rather than diverging. The effective cap is this value for PDF paths and `max_file_bytes` otherwise, applied by both the path check and `safe_read_file`. Documentation owner: `docs/config.md` § Local Workspace Defaults; architecture owner: `architecture/local-workspace.md` § Configuration.
3. **Local extraction and document.** Read bytes, then under `#[cfg(feature = "pdf")]` call `extract_pdf_text` with the resolved output budget (`max_chars` argument or `fetch.max_chars_default`, already capped by `max_chars_cap`), `PdfLimits` from `Config::fetch_limits()`, and options with no selected pages, no password, `include_media: false`, and OCR `Never`. Build the response with `content_type: Some("application/pdf")`, `document: Some(result.document)`, `text` and `lines` derived from the extracted text under the existing line-range and `max_chars` clamping, `language: None`, and `code_context: None`. Append the extractor's warnings, including page-classification strings; add no response field.
4. **Honest unavailability.** A build without the feature returns `ToolError::capability_unavailable(...)`, which renders the `capability_unavailable` code with the `pdf_not_compiled_in` message, the feature name, and `docs/features.md` § PDF Extraction; a feature-enabled build with `[fetch].pdf_enabled = false` returns `pdf_disabled`. Neither falls through to UTF-8 decoding. `PdfNoExtractableText` is mapped to an explicit-empty response with a warning naming the outcome rather than a hard failure; parse and encryption failures name their existing kind. No "failed to read file" misdiagnosis.
5. **Batch per-item shape.** Add one field to `BatchFetchItem::Web`, last and mirroring `web_fetch`: `#[serde(default, skip_serializing_if = "Option::is_none")] pdf: Option<PdfFetchOptions>`. `Repo` is unchanged. Absent `pdf` deserializes to today's behaviour; present, it accepts `{"pages": "1,3,7-10"}` plus the three existing sub-fields with unchanged semantics.
6. **Pre-I/O validation.** In the batch pre-validation loop, for a web item with `pdf.pages`, call `parse_pdf_page_spec(spec, state.config.fetch.pdf_max_pages)` under the feature and reject with the `item {i}` prefix. This is stricter than `web_fetch`, which parses after the body; `web_fetch` is not changed. Out-of-range needs the document page count, so it is necessarily post-download and fails only that item with `pdf_page_out_of_range`.
7. **Cache and telemetry threading.** Pass the item's `pdf.pages`, `pdf.pdf_ocr`, and `pdf.include_media` into the fetch options and into every batch `build_derived_key` call site so page-selected items get distinct keys and hit only their own entries. Add no counters; page selection affects `chars_returned` only through the returned text.

## Storage, protocol, and compatibility effects

- One additive `[local]` key with a default and range, and one additive optional object on the `batch_fetch` web item; existing configs resolve the key from its default, and `skip_serializing_if` keeps existing serialized payloads unchanged.
- `repo_fetch` returns a `document` for local PDFs that previously errored; no field is added or renamed, and `lines`/`text` remain populated.
- `renderer_version` is **not** bumped. Verified: local reads are not cached at all, and the batch keys this adds either already exist in `web_fetch`'s key space or only create new entries, so no stored document changes for an existing key. Caching local documents would change that and requires an explicit bump, not a silent one here.
- The internal Rust source break is confined to this crate: `BatchFetchItem::Web` has named fields and no `#[non_exhaustive]`, so every struct literal in `src/` and the test suites gains `pdf: None`; the MCP wire schema only gains an optional property.

## Ordered work packages

1. Add the `[local]` key, default, validation, and docs anchor; extend `tests/config_validation.rs`.
2. Exempt PDF from the path-level binary rejection and select the effective cap; update the `src/core/local.rs` unit tests, including the uppercase case.
3. Add the local PDF read and extraction path with the compiled-out and disabled branches, leaving the text path unchanged.
4. Map extraction outcomes into `repo_fetch` results and warnings; assert local trust, control-character stripping, and the injection scan.
5. Add `pdf: Option<PdfFetchOptions>` to `BatchFetchItem::Web`, update every struct literal for the variant, and add serde round-trip tests with and without `pdf`.
6. Add batch pre-I/O page validation, then thread the options into the fetch call and all derived-key sites.
7. Extend `tests/local_workspace_integration.rs` and `tests/batch_fetch_retrieval.rs` with the closure matrices, update the owning docs and `docs/test-inventory.md` counts, and append the `CHANGELOG.md` entry.

## Verification

- `cargo test --locked --features 'mock pdf' --test local_workspace_integration`
- `cargo test --locked --no-default-features --features mock --test local_workspace_integration`
- `cargo test --locked --features 'mock pdf' --test batch_fetch_retrieval`
- `cargo test --locked --all-features --test config_validation --test mcp_tools`
- `cargo test --locked --test docs_tool_names --test static_guards`
- `make check`; `make docs-check`

## Static guards and documentation updates

- `tests/docs_tool_names.rs` and `tests/static_guards.rs` (`stable_tool_registration_count_and_names`) stay green untouched: no tool or argument name changes and the count stays ten.
- `tests/static_guards.rs` `orchestration_module_size_ratchet` and `ordinary_module_size_ceiling_enforced` bound the edit. `src/fetch/pdf.rs` has 11 lines and 59 bytes of headroom, so the milestone calls the existing public extractor from the tool layer instead of growing it; if `src/mcp/tools/batch_fetch.rs` crosses 1,600 lines, extract a web-item helper rather than editing a ceiling.
- `docs/tool-matrix.md` batch row and its `web_fetch` PDF note gain per-URL `pdf.pages`; `architecture/mcp.md` § The 10 Tools mirrors it.
- `docs/features.md` § PDF Extraction states that local workspace reads and batch page selection use the same extractor and the same feature and `pdf_enabled` gates.
- `docs/config.md` § Local Workspace Defaults and `architecture/local-workspace.md` § Configuration own the new key; `docs/test-inventory.md` § Behavioral & Workflow Contracts counts are updated for the two extended suites. No suite is added, so `architecture/testing.md` needs no placement change.

## Acceptance and stop conditions

- A text PDF under an enabled root read through `repo_fetch` `host: "workspace"` returns text and a `FetchDocument` of kind pdf with local trust, normal sanitization, and no external framing.
- A PDF above the effective local cap returns the size outcome naming the configured value, never a UTF-8 or parse error.
- A build without the feature returns the capability-unavailable outcome naming the feature; a build with `[fetch].pdf_enabled = false` returns `pdf_disabled`.
- A non-PDF local file behaves exactly as today, including binary rejection and `max_file_bytes`.
- A batch web item with `pdf.pages` returns the selected pages and a document whose page markers match the selection; a non-PDF URL and `metadata_only` ignore the option exactly as `web_fetch` does.
- Invalid page syntax aborts before any fetch with `item {i}`; an out-of-range selection fails only that item and leaves other results and their order intact.
- Batch order, `index`, `stable_id`, `chars_returned`, and every telemetry counter are unchanged by the new field.
- Stop if per-URL selection would require a new tool, a renamed argument, or a non-additive schema change.
- Stop if the extractor cannot accept an in-memory byte slice without moving it out of the fetch subsystem: report the boundary question and hand it to a separate decision rather than restructuring inside this milestone.
- Stop if preserving indexing or repo-map behaviour would require changing `BINARY_EXTENSIONS` or `is_binary_extension`.
- Stop if the change cannot fit the module ceilings without a restructuring larger than the capability.

## Closure evidence

`plans/closure/pdf-ingestion-fidelity/008-status.md` records the candidate SHA, the local read matrix (supported, over budget, compiled out, disabled), the per-URL page-selection matrix (valid, invalid, out of range, mixed with non-PDF items, and interaction with `metadata_only`), the observed order and telemetry before and after (indices, stable IDs, character counts, counters), the unchanged ten-tool inventory with guard output, and the `renderer_version` no-bump evidence.
