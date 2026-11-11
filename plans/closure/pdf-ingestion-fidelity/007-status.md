# M007 Hostile-Input and Memory Bounds — Closure Status

Status: closing

Source plan: `plans/implementation/pdf-ingestion-fidelity/007-hostile-input-and-memory-bounds.md`

Roadmap: `plans/subsystems/pdf-ingestion-fidelity-roadmap.md`

Planning baseline: `9d4d16fc32500196a1b88554b898cd8eddc84731` (2026-11-11 UTC)

Implementation candidate: the uncommitted worktree on that baseline. No closure commit exists yet.

## Result

PDF ingestion has a measured decompression ceiling that runs before the parser:

- `[fetch].pdf_max_decompressed_bytes` (default 67,108,864, validated
  `1,048,576..=1,073,741,824`) reaches `FetchLimits` → `PdfLimits` → the scan.
- `src/fetch/pdf_stream.rs::scan_stream_budget` is the first statement of `extract_pdf_text`, before
  `Document::load_mem`. It is one forward, string- and comment-aware pass over the raw buffer. Per
  stream the allowance is `min(configured, remaining document allowance)`; per document it is the
  configured total.
- `FlateDecode` is inflated through a capped reader that never buffers more than the allowance plus one
  block, including `lopdf`'s raw-deflate retry. `ASCII85Decode` is decoded under the same cap, including
  the `z` shorthand's 1:4 expansion. A filter `lopdf` cannot decode contributes nothing when it is the
  first stage, and the stages decoded before it are counted when it follows one, because `lopdf`
  inflates those stages before returning `Unimplemented`. `LZWDecode` is refused as unmeasurable.
- Declared geometry is checked before allocation: an xref or object stream whose `/W` field width
  exceeds eight bytes or whose widest row exceeds the remaining allowance is refused, whether or not
  `/Type /XRef` is present, because `xref_and_trailer` decodes any first stream as an xref stream.
  A predictor row above the remaining allowance is refused for the same reason.
- A filtered stream whose `/Length` is an indirect reference is refused: `lopdf` resolves the
  reference and inflates the declared extent, and the `endstream` fallback can under-measure it when
  the compressed bytes contain that keyword.
- A breach is `FetchError::PdfDecompressionBoundExceeded { measured, budget }`, kind
  `pdf_decompression_bound_exceeded`. `measured` always exceeds `budget`: accounted bytes for a
  measurement breach, `budget + 1` for a structural refusal. No page metadata, text, or blocks are
  produced.

## Bomb matrix

| Shape | Test | Observed |
|---|---|---|
| Content-stream Flate bomb | `pdf_content_stream_bomb_is_refused` | refused, measured above the configured budget |
| Object-stream bomb | `pdf_object_stream_bomb_is_refused` | refused |
| Xref-stream bomb | `pdf_xref_stream_bomb_is_refused` | refused |
| Chained filters | `pdf_chained_filter_bomb_is_refused` | refused |
| Flate followed by an unknown filter | `pdf_flate_then_unknown_filter_bomb_is_refused` | refused |
| Xref `/W` geometry | `pdf_xref_width_geometry_bomb_is_refused` | refused before allocation |
| `LZWDecode` | `pdf_lzw_stream_is_refused` | refused as unmeasurable |
| Bound attribution | `pdf_bound_reports_the_configured_budget` | `budget` equals the configured value |
| Scan accounting | `pdf_scan_total_stays_within_the_configured_budget` | measured total within the configured budget |
| Property | `scan_total_never_exceeds_the_configured_budget`, `extraction_reports_only_the_configured_budget` | holds over arbitrary bytes |

The bomb fixtures are built at test time with `flate2` and a small configured budget, so no test
allocates the bomb's expansion.

## Encrypted documents

`authenticate_password` is gone. A document that declares `/Encrypt` returns `pdf_encrypted` before any
page access, with or without a caller password, because authentication never decrypted and ciphertext
cannot be measured by the scan. `pdf_password` stays in `PdfFetchOptions` for wire compatibility and is
documented as unsupported; the schema description grew by 158 bytes, which re-based
`tests/tool_surface_live.rs` (`73,064` definition bytes, `18,266` estimated tokens).
`pdf_encrypted_document_is_refused_without_and_with_a_password`,
`pdf_plain_document_is_unaffected_by_the_password_argument`, and
`pdf_compressed_document_still_extracts` cover the matrix.

## Baseline-document measurement

Both reference documents were fetched on 2026-11-11 and extracted under the default configuration
(25 pages, 50,000 characters, 64 MiB decompressed budget). Extraction output is unchanged from the
pre-change shape recorded in ADR-0008, and the new byte cap now admits both documents.

| Document | Bytes | Measured scan total | Scan share of budget | Chars | Non-empty lines | Max line | Quality |
|---|---:|---:|---:|---:|---:|---:|---:|
| arXiv 1706.03762 | 2,215,244 | 22,168,659 | 33% | 38,891 | 141 | 4,301 | 1.00 |
| RFC 9110 (25 pp) | 2,858,365 | 5,224,270 | 8% | 50,000 | 927 | 3,173 | 1.00 |

The line counts and maximum line lengths match the baseline measurement in ADR-0008 (141/4,301 and
928/3,173), which is the intended evidence that the bound did not change accepted extraction.

## Cache and deadline

`renderer_version` moved from 3 to 4 because the `/Encrypt` outcome changed.
`derived_key_renderer_version_isolates_pre_encryption_refusal_entries` and
`cache_derived_does_not_serve_pre_encryption_refusal_entries` prove a v3 entry is not served for a v4
key. Extraction is still neither offloaded nor timed out: the fetch deadline stops waiting at best and
cannot cancel a synchronous parse, so the byte bound, not the clock, is the allocation bound. That is
recorded rather than claimed as a deadline guarantee.

## Verification

- `cargo fmt --check` — clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — clean.
- `cargo check --locked --no-default-features` — clean.
- `cargo test --locked --all-features --lib fetch::` — 653 passed.
- `cargo test --locked --all-features --test adversarial_corpus` — 28 passed;
  `property_fetch_limits` 13; `web_fetch_integration` 85; `fetch_safety` 135.
- `cargo test --locked --all-features --test config_validation` — 79; `static_guards` 61;
  `docs_config_snippets` 2; `docs_tool_names` 2.
- `cargo test --locked --features pdf --test pdf_fidelity` — 15 passed, 1 ignored.
- `cargo test --locked --features mock --test adversarial_corpus --no-run` — compiles without the
  `pdf` feature.
- `./packaging/check-dependency-policy.sh` and `./packaging/check-repo-hygiene.sh` — ok.
- New static guard `pdf_inflation_enters_only_through_the_bounded_scanner` keeps non-test
  `src/fetch/pdf.rs` free of `decompressed_content` and `.decompress(` and requires the scan call.

## Deviations

1. The scanner lives in `src/fetch/pdf_stream.rs` (983 lines / 32,214 bytes), not `src/fetch/pdf.rs`,
   because that file had under 1 KB of ratchet headroom. `pdf.rs` is 2,037 lines / 69,433 bytes under
   the unchanged 2,100 / 70,000 ceiling.
2. Two extractor-level tests first written in `pdf.rs` moved to `tests/adversarial_corpus.rs` to stay
   under the ratchet; that suite also records the measured total.
3. The plan's "ASCII85 contributes at most four fifths of its input" is true for group encoding but not
   for the `z` shorthand; the scan decodes under the cap and counts the real output.
4. The plan's "other filters contribute zero" holds only when such a filter is first, because `lopdf`
   inflates preceding Flate stages before returning `Unimplemented`; the scan counts those stages.
5. A predictor-row geometry refusal was added beyond `/W`.
6. `flate2` is an optional normal dependency behind `pdf`; the dev-dependency entry remains so
   non-`pdf` test builds compile.

## Residual risk

A filtered stream with an indirect `/Length` is refused rather than measured. That is fail-closed but
narrows compatibility for producers that use indirect lengths with compressed streams; both reference
documents use direct lengths (0 indirect occurrences), and the rule is documented in
`docs/features.md` and `architecture/fetch.md`. The tracked `fuzz/Cargo.lock` is stale (it pins
eggsearch 0.3.7 with `reqwest`) and was left untouched; the fuzz target compiles offline but
`make fuzz-smoke` was not run in this session.
