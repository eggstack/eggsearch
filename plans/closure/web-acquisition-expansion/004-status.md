# Web Acquisition M004 — Document Formats Beyond PDF — Status

Status: closed
Date: 2026-11-25
Milestone: M004 — Document Formats Beyond PDF
Decision authority: ADR-0026 Decision 2; ADR-0007 (format artifact availability); ADR-0008 (extraction fidelity)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus this change set, uncommitted in the worktree
on `docs/user-facing-polish-20261123`. `documents` is a non-default feature,
so the shipped default feature set, the advertised tool count, and the default
fetch behaviour are unchanged.

- `src/fetch/documents/extract.rs` (1,317 lines): bounded extraction for
  CSV/TSV, EPUB, DOCX, XLSX, and PPTX, ZIP preflight, XML tokenisation, and the
  typed outcomes.
- `src/fetch/documents/tests.rs` (293 lines): seven offline unit cases with
  generated archives.
- `src/fetch/documents/mod.rs` (7 lines): the feature-gated surface.
- `src/core/document.rs` (664 lines): `DocumentFormat`, `DocumentPartKind`,
  and `DocumentPart`, plus `format`, `parts`, and `parts_total` on
  `FetchDocument`. `DocumentKind` is unchanged, so no renderer match arm moved.
- `src/fetch/client.rs` (2,290 lines, ratchet 2,600): document detection in the
  pre-body gate, a document body cap, and the early-return extraction path that
  mirrors the PDF path.
- `src/fetch/limits.rs` (1,263 lines): the 16 MiB document body cap.
- `src/fetch/types.rs` (437 lines): `DocumentEmpty`, `DocumentMalformed`,
  `DocumentUnsupported`, and `DocumentEncrypted`.
- `src/fetch/detect.rs` (761 lines): `detect_document_format` by content
  type and extension.
- `src/core/focus.rs`, `src/fetch/pdf.rs` (2,042 lines), and
  `src/fetch/browser/navigate.rs`: initialisers for the new `FetchDocument`
  fields only.
- `tests/documents.rs` (new): seven feature-on cases and two feature-off cases.
- `bindings/python/python/eggsearch/types.py`: `DocumentFormat`,
  `DocumentPartKind`, `DocumentPart`, and the new `FetchDocument` fields.

## Dependency evidence

| Crate | Pin | MSRV | Licence | Note |
|---|---|---|---|---|
| `zip` | `=9.8.1` | 1.89 | MIT | `default-features = false`, `deflate-flate2` (Rust backend) |
| `calamine` | `0.38` | 1.88 | MIT | `default-features = false`, `dates` |
| `similar` | `2.7` | 1.60 | Apache-2.0 | Added for M003, already in the lock file |

`cargo tree --locked --all-features -i zip@9.8.1` shows eggsearch and calamine
sharing one archive major, so the recorded fallback (defer XLSX and use
`zip 2.6`) was not needed.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Each supported format yields deterministic parts and text | `docx_url_returns_ordered_parts_deterministically`, `epub_url_returns_parts_in_spine_order`, `csv_url_returns_fixed_row_blocks`, plus the unit cases for all five formats | pass |
| Part ordering never follows ZIP directory order | EPUB spine order, DOCX body order, workbook sheet order, and `presentation.xml` slide order asserted in the unit cases and the DOCX/EPUB integration cases | pass |
| Malformed, empty, encrypted, and unsupported inputs are distinguishable | `malformed_container_is_distinguishable`, `encrypted_container_is_distinguishable`, and `malformed_empty_encrypted_and_high_ratio_inputs_are_distinguished` | pass |
| A decompression bomb is refused by the size or ratio guard | `preflight_zip` plus the high-ratio case in `malformed_empty_encrypted_and_high_ratio_inputs_are_distinguished` | pass |
| Feature off leaves default behaviour unchanged | `docx_without_the_feature_keeps_the_unsupported_outcome`, `csv_without_the_feature_keeps_the_table_preview` | pass |
| No DTD or external entity is resolved | `xml_rejects_dtds_and_part_text_budget_marks_truncation` | pass |
| Text and part budgets mark truncation rather than failing | same case, which now distinguishes a document-budget clip from a return-window clip | pass |

## Defects found and corrected during integration

- An extensionless URL whose last path segment is a legacy Office name
  (`/doc`, `/ppt`, `/xls`) was classified as that legacy format, because the
  extension was taken from the final path segment whether or not it followed a
  dot. Any ordinary page at such a path was then refused. The extension now
  requires a dot, and an HTML representation is never treated as a legacy
  document.
- Known-unsupported document types were rejected by the pre-body gate before
  the typed outcome could be produced, so legacy binary Office surfaced as
  `unsupported_content_type` instead of `document_unsupported`. The gate now
  admits them and the post-body check names the specific outcome.
- The pre-body content-type gate still rejected `application/epub+zip` and the
  OOXML types before the body was read, so EPUB, DOCX, XLSX, and PPTX failed with
  `unsupported_content_type` even with the feature compiled in. The new
  feature-on integration cases caught it; the gate now admits a detected
  document format, and the feature-off rejection is unchanged.
- `src/fetch/documents/extract.rs` carried a `drop_non_drop` call and a
  `len() == 0` comparison that fail `-D warnings` under the pinned 1.89
  toolchain. Both were corrected.
- The truncation unit case asserted that a document-budget clip also sets the
  return-window flag. Those are distinct budgets, so the case now asserts each
  separately.

## Deviations and limits

- **Part selection is deferred.** The plan assumed an existing document
  `offset`/`limit` selector. The web-fetch request has none (only
  `pdf.pages`), and adding one would change the request shape for every web
  item and re-pin the advertised schemas. This version returns every part and
  reports `parts_total`. The plan's Contract section records the correction.
- Fixtures are generated in memory rather than committed as binaries, so the
  corpus cannot drift from the parser and no binary blobs enter the tree.
- No package-size measurement is recorded. `documents` stays non-default, so
  the measurement is required only before a default-set proposal.
- ODF, legacy binary Office, and RTF remain explicitly unsupported.

## Verification on this candidate

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked --all-features` | pass |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked` (feature off) | pass |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test documents` | pass: 7 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --test documents` (feature off) | pass: 2 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib fetch::documents` | pass: 7 passed |
| `RUSTUP_TOOLCHAIN=1.89.0 cargo clippy --locked --all-targets --all-features -- -D warnings` | pass |
| `RUSTUP_TOOLCHAIN=1.89.0 make check` | pass |
| `git diff --check` | pass |

## Limitations

- No OCR, image understanding, or scanned-document recovery.
- No layout blocks, bounding boxes, forms, comments, or tracked changes.
- No macro execution, embedded objects, or external relationship resolution.
- No document writing or conversion.
- Local arbitrary-path parsing stays inside the existing authorized workspace rules.
- Detection by container introspection applies once a representation is admitted;
  a container served only as `application/octet-stream` at a non-format URL is
  still refused before the body is read.
