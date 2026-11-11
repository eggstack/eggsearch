# M001 PDF Fetch Contract and Budgets — Closure Status

Status: closing

Source plan: `plans/implementation/pdf-ingestion-fidelity/001-pdf-fetch-contract-and-budgets.md`

Roadmap: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md`

Planning baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731` (2026-11-11 UTC)

Implementation candidate: the uncommitted worktree on that baseline. No closure commit exists yet; this
record, the roadmap status, and the registry row move together when the candidate is committed.

## Result

The PDF fetch boundary now reports what actually happened:

- `[fetch].pdf_max_bytes` (default 16,777,216, validated `1..=268_435_456`) gives PDFs a body budget
  separate from the generic `max_bytes`. `FetchLimits::body_cap(is_pdf)` selects it for a response
  identified as PDF by Content-Type or a `.pdf` path; every other response keeps `max_bytes`.
- A PDF refused by the `Content-Length` pre-check, or truncated at the effective cap, returns
  `FetchError::ContentTooLarge` naming the configured cap. A truncated body never reaches the parser,
  so a size problem can no longer surface as `pdf_parse_error`.
- `count_page_images` runs for every extracted page. An image-only page classifies as
  `ScannedOrImageOnly` (0.10) at default settings, and `include_media` only decides whether
  `image_count` is populated in page metadata.
- `pdf_ocr = "auto"` extracts text and adds a per-page `PdfOcrUnavailable` structured warning on
  scanned pages; `always` still fails closed with `pdf_ocr_unavailable`.
- `detect_from_content_type` gained the `application/pdf` arm, so the document classifier and the
  fetch gate agree.
- `pdf_not_compiled_in` now names the feature, `docs/features.md#pdf-extraction`, and
  `cargo build --features pdf`.

## Cap-selection matrix

| Response | Cap applied | Observed outcome |
|---|---|---|
| PDF by Content-Type or `.pdf`, length above `max_bytes`, below `pdf_max_bytes` | `pdf_max_bytes` | fetched and extracted |
| PDF by Content-Type, declared length above `pdf_max_bytes` | `pdf_max_bytes` | `content_too_large` naming the cap, no parse attempt |
| PDF by Content-Type, chunked and truncated at `pdf_max_bytes` | `pdf_max_bytes` | `content_too_large`, no parse attempt |
| PDF recognised only by `%PDF-` magic | `max_bytes` | unchanged; the cap is fixed before the first bytes |
| Non-PDF above `max_bytes` | `max_bytes` | unchanged, including chunked truncation without an error |

## Page classification and OCR

| Case | Before | After |
|---|---|---|
| Image-only page, `include_media = false` | `Blank`, document answered `pdf_no_extractable_text` | `ScannedOrImageOnly` with the scanned warning; `include_media=false` leaves `image_count` unset |
| Image-only page, `include_media = true` | `ScannedOrImageOnly` plus counts | same classification, counts populated |
| Text PDF, `pdf_ocr = "never"` | extracts | extracts |
| Text PDF, `pdf_ocr = "auto"` | `pdf_ocr_unavailable` before any page was read | extracts |
| Image-only PDF, `pdf_ocr = "auto"` | `pdf_ocr_unavailable` | explicit-empty document, scanned classification, per-page OCR-unavailable warning |
| Any PDF, `pdf_ocr = "always"` | `pdf_ocr_unavailable` | unchanged |

## Cache and capability reporting

`renderer_version` moved from 2 to 3 because page classification is part of the cached derived
document. `derived_key_renderer_version_isolates_pre_pdf_classification_entries` and
`cache_derived_does_not_serve_pre_encryption_refusal_entries` prove a pre-bump entry is not served.
`provider_status.pdf_capabilities` keeps its `{ compiled, configured, usable, layout, ocr }` shape and
is asserted against both `pdf_enabled` values; the byte budget is a limit, not a usability term, so it
is documented in configuration rather than surfaced there (M006 owns the capability alignment).

## Module split

The milestone changed page-quality classification, and `src/fetch/pdf.rs` had 11 lines and 59 bytes of
headroom under its ratchet. `classify_page_quality`, `count_cid_tokens`, `count_page_images`, and their
constants and tests moved to `src/fetch/pdf_quality.rs` (172 lines / 5,198 bytes). `src/fetch/pdf.rs`
is 2,037 lines / 69,433 bytes against the unchanged 2,100-line / 70,000-byte ceiling.

## Verification

- `cargo fmt --check` — clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — clean.
- `cargo check --locked --no-default-features` — clean.
- `cargo test --locked --all-features --lib fetch::` — 631 passed at the M001 candidate.
- `config_validation` 76, `mcp_tools` 6, `web_fetch_integration` 85, `fetch_safety` 135,
  `adversarial_corpus` 16, `static_guards` 60, `docs_config_snippets` 2 passed at the M001 candidate.
- New behavioural coverage: `web_fetch_pdf_above_max_bytes_within_pdf_budget_extracts`,
  `web_fetch_pdf_above_pdf_budget_returns_size_outcome`,
  `pdf_chunked_body_truncated_at_budget_returns_size_outcome`,
  `magic_sniffed_pdf_without_declared_signal_stays_under_max_bytes`,
  `non_pdf_chunked_body_still_truncates_without_error`,
  `non_pdf_declared_length_above_max_bytes_keeps_generic_cap`,
  `ocr_policy_auto_extracts_text_pdf`, `ocr_policy_auto_records_scanned_pages_without_failing`,
  `image_only_pdf_classified_as_scanned_without_include_media`,
  `include_media_changes_only_reported_counts`, `detect_from_content_type_pdf_matches_fetch_gate`,
  `provider_status_pdf_capabilities_track_feature_and_config`, `toml_absent_pdf_max_bytes_uses_default`,
  `validate_rejects_pdf_max_bytes_out_of_range`.

## Deviations

1. The cap selector lives in `FetchLimits::body_cap` (`limits.rs`) rather than `client.rs`, because
   `client.rs` is at a 2,600-line ratchet and the change pushed it over. No ceiling was raised.
2. `fetch_conditional` keeps `max_bytes`: both nullable callers discard non-304 bodies, so widening
   that read would download a PDF only to throw it away. The extraction boundary selects the PDF cap.
3. Truncated PDFs are refused before the `metadata_only` branch, so the size outcome is uniform.
4. `provider_status.pdf_capabilities` shape is unchanged; see above.
5. `docs/config.md` TOML snippets omit every `pdf_*` key, so `pdf_max_bytes` is documented in the
   defaults table and prose rather than added to a snippet that would then need to list the rest.

## Residual risk

A PDF that is served without a PDF Content-Type and without a `.pdf` path is still read under
`max_bytes`; the magic sniff runs after the body cap has been fixed. This is deliberate, documented in
`docs/config.md` and `architecture/fetch.md`, and not widened by this milestone.
